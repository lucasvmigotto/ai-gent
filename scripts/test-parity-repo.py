#!/usr/bin/env python3
"""Parity gate for `ai-gent repo` (spec 037 Group C, decision D10).

The cases in rust/fixtures/repo/*.json are the tests of plugins/repo/scripts/test_reporun.py turned into
data: a git repository with a remote, a pass store, fake platform CLIs (scripts/fake_tool.py or the Rust
twin), a local HTTP stub standing in for each API, the commands to run, and what reporun.py printed,
exited with and sent to the stub. This script is the oracle runner; `cargo test -p ai-gent --test
parity_repo` is the Rust runner over the same files.

  scripts/test-parity-repo.py              run every case through reporun.py (the oracle)
  scripts/test-parity-repo.py --bin PATH   run every case through the ai-gent binary instead
  scripts/test-parity-repo.py --bless      regenerate the fixtures from the Python tool, then review the diff

--bless refuses to run once reporun.py is gone (deleted at the 3.0.0 cutover; the fixtures then stay).
Canary (FR-A5): every run's stdout, stderr and created files are checked for the fake token, which only
the child's environment and the stub's request may carry. Network: only the loopback stub; never GitHub.

Fixture keys: name, derived_from, remote (git remote URL or null), git ([[git args]] run first in the
repo), store ({entry: text}), tools ({name: {"rules": [...]}}), env ({NAME: value, "{stub}" expands to the
stub URL, null removes}), stub (bool), files ({path under the root: text}), runs ([{argv, stdin, git,
exit, stdout, stderr_has, seen}]), expect_files. `<ROOT>` is the temp root; `<STUB>` the stub address.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY_TOOL = ROOT / "plugins" / "repo" / "scripts" / "reporun.py"
FAKE_PY = ROOT / "scripts" / "fake_tool.py"
FIXTURES = ROOT / "rust" / "fixtures" / "repo"
TOKEN = "tok-3f9a1c77e0"
SEEN_HEADERS = ("Authorization", "Content-Type", "Accept")


# ------------------------------------------------------------------ the stub

class Stub(BaseHTTPRequestHandler):
    seen = []
    counter = 100

    def log_message(self, *a):
        pass

    def reply(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        hdrs = {h: self.headers[h] for h in SEEN_HEADERS if self.headers.get(h)}
        type(self).seen.append({"method": self.command, "path": self.path, "headers": hdrs,
                                "body": body.decode("utf-8", "replace")})
        path = self.path.split("?")[0]
        if self.path.startswith("/echo-token"):
            return self.send(401, {"message": f"bad credentials {TOKEN}"})
        if self.command == "POST" and path.endswith("/issues"):
            type(self).counter += 1
            n = type(self).counter
            return self.send(201, {"number": n, "id": 9000 + n, "html_url": f"https://github.com/o/r/issues/{n}"})
        if self.path.startswith("/slow"):
            import time
            time.sleep(1.5)
            return self.send(200, {"slow": True})
        if self.path.startswith("/fail"):
            return self.send(422, {"message": "Validation Failed"})
        if self.path.startswith("/page1"):
            port = self.server.server_address[1]
            return self.send(200, {"values": [1, 2], "next": f"http://127.0.0.1:{port}/page2"})
        if self.path.startswith("/page2"):
            return self.send(200, {"values": [3]})
        self.send(201 if self.command == "POST" else 200, {"ok": True})

    def send(self, status, payload):
        raw = json.dumps(payload, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    do_GET = do_POST = do_PATCH = do_PUT = do_DELETE = reply


# --------------------------------------------------------------- case builder

class Scn:
    def __init__(self, remote="git@github.com:o/r.git", token=True, stub=True, tools=None):
        self.c = {"remote": remote, "git": [], "store": {}, "tools": {} if tools is None else tools, "env": {},
                  "stub": stub, "files": {}, "runs": []}
        if token:
            self.entry("ai-gent/github/pat", f"{TOKEN}\n")

    def entry(self, name, text):
        self.c["store"][name] = text
        return self

    def tool(self, name, rules=None):
        self.c["tools"][name] = {"rules": rules or []}
        return self

    def put(self, rel, text):
        self.c["files"][rel] = text
        return self

    def env(self, **kw):
        self.c["env"].update(kw)
        return self

    def git(self, *args):
        self.c["git"].append(list(args))
        return self

    def run(self, *argv, stdin=None, stderr_has=(), git=None):
        r = {"argv": list(argv), "stdin": stdin, "stderr_has": list(stderr_has)}
        if git:
            r["git"] = [list(g) for g in git]
        self.c["runs"].append(r)
        return self


FIELD_LIST = json.dumps({"fields": [
    {"id": "PVTF_1", "name": "Status", "type": "ProjectV2SingleSelectField", "options": [
        {"id": "OPT_TODO", "name": "To do"}, {"id": "OPT_DOING", "name": "In Progress"},
        {"id": "OPT_REVIEW", "name": "In Review"}, {"id": "OPT_DONE", "name": "Done"}]},
    {"id": "PVTF_2", "name": "Title", "type": "ProjectV2Field"}]})
FIELD_LIST_ODD = json.dumps({"fields": [
    {"id": "PVTF_1", "name": "Status", "type": "ProjectV2SingleSelectField",
     "options": [{"id": "OPT_TODO", "name": "To do"}, {"id": "OPT_DOING", "name": "In-Progress"},
                 {"id": "OPT_DONE", "name": "Done ✅"}]},
    {"id": "PVTF_2", "name": "Title", "type": "ProjectV2Field"}]})


def gh_project(board=FIELD_LIST):
    return [
        {"args": "project field-list", "stdout": board + "\n"},
        {"args": "project item-edit", "capture": "<ROOT>/item-edit.args", "stdout": "token: {env:GH_TOKEN}\n"},
        {"args": "issue view", "stdout": '{"url": "https://github.com/o/r/issues/5"}\n'},
    ]


def cases():
    out = {}

    def add(name, derived, s):
        out[name] = (derived, s)

    # RemoteTest.test_platforms / test_forced_platform, through `detect --json`.
    urls = ["git@github.com:o/r.git", "https://github.com/o/r", "https://acme.ghe.com/o/r.git",
            "https://org@dev.azure.com/org/My%20Proj/_git/app", "git@ssh.dev.azure.com:v3/org/proj/app",
            "https://org.visualstudio.com/DefaultCollection/proj/_git/app", "https://dev.azure.com/org/_git/app",
            "git@gitlab.com:grp/sub/proj.git", "git@bitbucket.org:ws/app.git", "https://codeberg.org/o/r.git",
            "git@gitea.example.com:o/r.git", "https://git.example.com/a/b",
            "https://user:pw-should-not-matter@github.com/o/r.git", "ssh://git@github.com:22/o/r.git"]
    s = Scn(token=False, stub=False)
    for u in urls:
        s.run("detect", "--json", git=[["remote", "set-url", "origin", u]])
        s.run("detect", git=[["remote", "set-url", "origin", u]])
    s.run("detect", "--json", git=[["remote", "set-url", "origin", "https://code.example.com/a/b"],
                                   ["config", "ai-gent.platform", "gitlab"]])
    add("detect-remote-urls", "RemoteTest::test_platforms, ::test_forced_platform", s)
    s = Scn(remote=None, token=False, stub=False).run("detect", "--json").run("detect")
    add("detect-no-remote", "reporun.detect_repo (no remote)", s)

    # EntryTest
    s = Scn()
    s.run("detect", "--json")
    s.run("detect")
    add("detect-never-decrypts", "EntryTest::test_detect_never_decrypts", s)
    s = Scn().tool("gh", [{"args": "auth status", "exit": 0}])
    s.run("detect", "--json").run("detect")
    add("detect-cli-installed-and-signed-in", "reporun.cli_state", s)
    s = Scn(token=False).entry("ai-gent/jira/token", f"{TOKEN}\nemail: me@example.com\nurl: https://acme.atlassian.net/\n")
    s.run("detect", "--json").run("detect")
    add("detect-jira-from-the-entry", "reporun.cmd_detect (jira)", s)
    s = Scn()
    s.run("api", "github", "GET", "/x", "--entry", "ai-gent/db/shop-dev")
    s.run("api", "github", "GET", "/x", "--entry", "personal/bank")
    s.run("api", "github", "GET", "/x", "--entry", "ai-gent/../x")
    s.run("api", "github", "GET", "/x", "--entry", "ai-gent/github/other")
    add("entry-rules", "EntryTest::test_entry_rules", s)

    # ApiTest
    s = Scn()
    s.run("api", "github", "GET", "/repos/{owner}/{repo}/issues", "--query", "state=open")
    s.run("api", "github", "GET", "/repos/o/r", "--query", "a=b c", "--query", "a=z", "--query", "novalue")
    add("api-github-bearer-and-placeholders", "ApiTest::test_github_bearer_and_placeholders", s)
    s = Scn(token=False)
    s.run("api", "github", "GET", "/repos/o/r")
    add("api-github-public-without-a-token", "reporun.api_call (github needs no entry)", s)
    s = Scn().put("b.json", '{"title": "x"}')
    s.run("api", "github", "POST", "/repos/o/r/issues", "--data", "<ROOT>/b.json")
    s.run("api", "github", "POST", "/repos/o/r/issues", "--data", "<ROOT>/b.json", "--write")
    s.run("api", "github", "POST", "/repos/o/r/issues", "--data", "-", "--write", stdin='{"title": "stdin"}')
    s.run("api", "github", "BREW", "/x")
    add("api-write-needs-flag", "ApiTest::test_write_needs_flag", s)
    s = Scn(remote="https://dev.azure.com/org/proj/_git/app", token=False).entry("ai-gent/azure-devops/pat", f"{TOKEN}\n")
    s.put("w.json", '[{"op": "add", "path": "/fields/System.Title", "value": "t"}]')
    s.run("api", "azure-devops", "GET", "/{project}/_apis/wit/workitems/12")
    s.run("api", "azure-devops", "POST", "/{project}/_apis/wit/wiql", "--data", "<ROOT>/w.json")
    s.run("api", "azure-devops", "PATCH", "/proj/_apis/wit/workitems/12", "--data", "<ROOT>/w.json", "--write")
    s.run("api", "azure-devops", "POST", "/proj/_apis/wit/workitems/$Task", "--data", "<ROOT>/w.json", "--write")
    add("api-azure-basic-and-api-version", "ApiTest::test_azure_basic_and_api_version", s)
    s = Scn(remote="https://codeberg.org/o/r.git", token=False).entry("ai-gent/forgejo/pat", f"{TOKEN}\n")
    s.put("m.json", '{"Do": "merge"}')
    s.run("api", "forgejo", "GET", "/repos/{owner}/{repo}/issues")
    s.run("api", "forgejo", "POST", "/repos/o/r/pulls/3/merge", "--data", "<ROOT>/m.json")
    add("api-forgejo-token-header-and-repo-path", "ApiTest::test_forgejo_token_header_and_repo_path", s)
    s = Scn(token=False).tool("gh")
    s.run("cli", "forgejo", "--", "issue", "list")
    s.run("cli", "bitbucket", "--", "x")
    add("cli-forgejo-has-no-cli", "ApiTest::test_forgejo_has_no_cli", s)
    s = Scn(token=False).entry("ai-gent/jira/token", f"{TOKEN}\nemail: me@example.com\n")
    s.put("q.json", '{"jql": "project = KEY"}')
    s.run("api", "jira", "POST", "/rest/api/3/search/jql", "--data", "<ROOT>/q.json")
    s.run("api", "jira", "POST", "/rest/api/3/issue", "--data", "<ROOT>/q.json")
    s.run("api", "jira", "GET", "/rest/api/3/myself")
    add("api-jira-basic-with-email", "ApiTest::test_jira_basic_with_email", s)
    s = Scn(token=False).entry("ai-gent/jira/token", f"{TOKEN}\n")
    s.env(REPORUN_TEST_BASE=None, JIRA_URL=None)
    s.run("api", "jira", "GET", "/rest/api/3/myself")
    add("api-jira-without-a-url", "reporun.api_base (jira)", s)
    s = Scn(remote="git@gitlab.com:grp/sub/proj.git", token=False).entry("ai-gent/gitlab/pat", f"{TOKEN}\n")
    s.run("api", "gitlab", "GET", "/projects/{project_path}/issues")
    s.run("api", "gitlab", "GET", "/projects/{nope}/issues")
    add("api-gitlab-project-path-encoded", "ApiTest::test_gitlab_project_path_encoded", s)
    s = Scn(token=False).entry("ai-gent/bitbucket/token", f"{TOKEN}\n")
    s.run("api", "bitbucket", "GET", "/page1", "--paginate")
    s.run("api", "bitbucket", "GET", "/page1", "--paginate", "--max-pages", "1", stderr_has=["stopped after 1 pages"])
    s.run("api", "bitbucket", "GET", "/page1")
    add("api-bitbucket-pagination", "ApiTest::test_bitbucket_pagination", s)
    s = Scn()
    s.run("api", "github", "GET", "https://evil.example.com/x")
    s.run("api", "github", "GET", "http://127.0.0.1:1/x")
    add("api-other-hosts-refused", "ApiTest::test_other_hosts_refused", s)
    s = Scn()
    s.run("api", "github", "GET", "/echo-token", stderr_has=["<token>", "HTTP 401"])
    s.run("api", "github", "GET", "/fail", stderr_has=["HTTP 422"])
    add("api-errors-redacted", "ApiTest::test_errors_redacted", s)
    s = Scn(token=False)
    s.run("api", "gitlab", "GET", "/projects", stderr_has=["pass insert -m ai-gent/gitlab/pat"])
    s.run("api", "github", "GET", "/x", "--entry", "ai-gent/github/other", stderr_has=["pass insert -m ai-gent/github/other"])
    add("api-missing-entry", "ApiTest::test_missing_entry", s)

    # CliTest
    s = Scn().tool("gh")
    s.run("cli", "github", "--", "pr", "list", stderr_has=["token from pass (ai-gent/github/pat)"])
    s.run("cli", "github", "pr", "view", "3")
    add("cli-token-in-env-only-and-redacted", "CliTest::test_token_in_env_only_and_redacted", s)
    s = Scn().tool("gh")
    for argv in (["auth", "token"], ["alias", "set", "x", "!env"], ["pr", "list", "--debug"],
                 ["extension", "install", "x/y"], ["pr", "merge", "3"], ["pr", "list", "--show-token"]):
        s.run("cli", "github", "--", *argv)
    add("cli-refusals", "CliTest::test_refusals", s)
    s = Scn(token=False).tool("gh")
    s.run("cli", "github", "--write", "--", "pr", "merge", "3", stderr_has=["token from the CLI's own login"])
    add("cli-write-with-flag", "CliTest::test_write_with_flag", s)
    s = Scn()
    s.run("cli", "github", "--", "pr", "list", stderr_has=["isn't installed"])
    add("cli-not-installed", "reporun.prepare_cli (cli missing)", s)
    s = Scn().tool("gh", [{"args": "pr list", "stdout": "out token={env:GH_TOKEN}\n", "stderr": "err {env:GH_TOKEN}\n", "exit": 4}])
    s.run("cli", "github", "--", "pr", "list")
    s.run("batch", "<ROOT>/b.json", "--write")
    s.put("b.json", json.dumps({"platform": "github", "steps": [{"name": "a", "cli": ["pr", "list"]}]}))
    add("cli-child-status-and-stderr-redacted", "reporun.cmd_cli / run_cli_capture", s)

    # Classification (ClassifyTest) through batch --dry-run
    cli_reads = {"github": ["pr list", "pr view 3 --comments", "pr checks 3", "issue list --search bug", "api repos/o/r/pulls", "run watch 5"],
                 "azure-devops": ["boards work-item show --id 3", "boards query --wiql x", "repos pr list", "devops invoke --area wit"],
                 "gitlab": ["mr view 2", "issue list"], "jira": ["issue list", "issue view X-1"]}
    cli_writes = {"github": ["pr create --title list", "issue comment 5 --body x", "pr merge 3 --squash", "api repos/o/r/issues -f title=x",
                             "api -X PATCH repos/o/r/issues/1", "-R o/r pr list", "run rerun 5", "label create status",
                             "issue edit 12 view", "pr comment 3 list"],
                  "azure-devops": ["boards work-item update --id 3 --state Done", "devops invoke --area wit --http-method PATCH"],
                  "gitlab": ["mr merge 2", "issue note 4 show"], "jira": ["issue move X-1 Done", "issue comment add KEY-1 list"]}
    s = Scn(token=False, stub=False)
    for plat in cli_reads:
        steps = [{"name": f"r{i}", "cli": w.split()} for i, w in enumerate(cli_reads[plat])]
        steps += [{"name": f"w{i}", "cli": w.split()} for i, w in enumerate(cli_writes[plat])]
        s.put(f"{plat}.json", json.dumps({"platform": plat, "steps": steps}))
        s.run("batch", f"<ROOT>/{plat}.json", "--dry-run")
    add("classify-cli-through-batch-dry-run", "ClassifyTest::test_cli", s)
    s = Scn(token=False, stub=False)
    api_steps = {
        "github": [("GET", "/repos/o/r/issues", None), ("POST", "/graphql", {"query": "query { viewer { login } }"}),
                   ("POST", "/graphql", {"query": "mutation { x }"}), ("POST", "/repos/o/r/issues", {}), ("HEAD", "/x", None)],
        "azure-devops": [("POST", "/proj/_apis/wit/wiql", {}), ("POST", "/proj/_apis/wit/wiql/abc", {}), ("PATCH", "/proj/_apis/wit/workitems/1", {})],
        "jira": [("POST", "/rest/api/3/search/jql", {}), ("POST", "/rest/api/3/search", {}), ("POST", "/rest/api/3/issue", {}),
                 ("POST", "/rest/agile/1.0/board/5/search", {}), ("POST", "/rest/agile/1.0/search", {})],
        "bitbucket": [("DELETE", "/repositories/w/r/pullrequests/1", None), ("POST", "/graphql", {})],
        "gitlab": [("POST", "/api/graphql", {"query": "{ a }"}), ("POST", "/graphql", {"query": "mutation { a }"}), ("PUT", "/x", {})],
    }
    for plat, steps in api_steps.items():
        body = []
        for i, (m, p, d) in enumerate(steps):
            a = {"method": m, "path": p}
            if d is not None:
                a["data"] = d
            body.append({"name": f"s{i}", "api": a})
        s.put(f"{plat}.json", json.dumps({"platform": plat, "steps": body}))
        s.run("batch", f"<ROOT>/{plat}.json", "--dry-run")
    add("classify-api-through-batch-dry-run", "ClassifyTest::test_api", s)

    # ProjectStatusTest
    def ps(board=FIELD_LIST):
        return Scn().tool("gh", gh_project(board))
    url = "https://github.com/o/r/issues/5"
    s = ps()
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "doing")
    add("project-status-refused-without-write", "ProjectStatusTest::test_refused_without_write", s)
    s = ps()
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "doing", "--write")
    add("project-status-state-mapped-and-written", "ProjectStatusTest::test_state_mapped_and_written", s)
    s = ps()
    s.run("project-status", "1", "--owner", "me", "--url", url, "--value", "Blocked", "--write")
    add("project-status-explicit-value-bypasses-mapping", "ProjectStatusTest::test_explicit_value_bypasses_mapping", s)
    s = ps()
    s.run("project-status", "1", "--url", url, "--state", "done", "--write")
    add("project-status-owner-defaults-to-remote", "ProjectStatusTest::test_owner_defaults_to_remote", s)
    s = ps()
    s.run("project-status", "1", "--owner", "me", "--url", url, "--field", "Priority", "--state", "todo", "--write", stderr_has=["Priority"])
    s.run("project-status", "1", "--owner", "me", "--url", url, "--field", "Title", "--state", "todo", "--write", stderr_has=["single-select"])
    add("project-status-unknown-field-fails", "ProjectStatusTest::test_unknown_field_fails, ::test_field_missing, ::test_field_without_options_rejected", s)
    s = ps()
    s.run("project-status", "1", "--owner", "me", "--url", url)
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "bogus")
    add("project-status-needs-state-or-value", "ProjectStatusTest::test_needs_state_or_value", s)
    s = ps(FIELD_LIST_ODD)
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "doing", "--write")
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "done", "--write")
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "review", "--write", stderr_has=["--value", "To do"])
    s.run("project-status", "1", "--owner", "me", "--url", url, "--state", "todo", "--field", "status", "--write")
    add("project-status-alias-and-punctuation-match", "ProjectStatusTest::test_alias_match, ::test_punctuation_insensitive_match, ::test_no_match_lists_options, ::test_field_found_case_insensitively", s)
    s = Scn(remote=None).tool("gh", gh_project())
    s.run("project-status", "1", "--url", url, "--state", "done", "--write", stderr_has=["no project owner"])
    add("project-status-no-owner", "reporun.project_status_call (no owner)", s)
    plan = lambda steps, plat="github": json.dumps({"platform": plat, "steps": steps})  # noqa: E731
    s = ps().put("b.json", plan([{"name": "status", "project_status": {"number": 1, "owner": "me", "url": url, "state": "review"}}]))
    s.run("batch", "<ROOT>/b.json", stderr_has=["1 of 1 steps"])
    s.run("batch", "<ROOT>/b.json", "--dry-run")
    add("project-status-batch-step-needs-write", "ProjectStatusTest::test_batch_step_needs_write", s)
    s = ps().put("b.json", plan([
        {"name": "issue", "cli": ["issue", "view", "5", "--json", "url"]},
        {"name": "status", "project_status": {"number": 1, "owner": "me", "url": "{{issue.url}}", "state": "review"}}]))
    s.run("batch", "<ROOT>/b.json", "--write", "--out", "<ROOT>/out.json")
    add("project-status-batch-step-written-and-resolves-references", "ProjectStatusTest::test_batch_step_written_and_resolves_references", s)
    s = ps().put("b.json", plan([{"name": "status", "project_status": {"number": 1, "owner": "me", "url": "x", "state": "todo"}}], "gitlab"))
    s.run("batch", "<ROOT>/b.json", stderr_has=["GitHub-only"])
    add("project-status-batch-rejects-other-platforms", "ProjectStatusTest::test_batch_rejects_project_status_on_other_platforms", s)

    # BatchTest
    steps = [
        {"name": "feature", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues", "data": {"title": "[012] Export"}},
         "record": {"key": "feature", "ref": "#{{self.number}}", "id": "{{self.id}}", "url": "{{self.html_url}}", "state": "todo"}},
        {"name": "US1", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues", "data": {"title": "[012] US1: CSV"}},
         "record": {"key": "US1", "ref": "#{{self.number}}", "id": "{{self.id}}", "state": "todo"}},
        {"name": "US1-parent", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues/{{feature.number}}/sub_issues",
                                       "data": {"sub_issue_id": "{{US1.id}}"}}},
    ]
    s = Scn().put("b.json", plan(steps))
    s.run("batch", "<ROOT>/b.json", stderr_has=["3 of 3 steps"])
    add("batch-needs-write-once", "BatchTest::test_needs_write_once", s)
    s = Scn(token=False).put("b.json", plan(steps))
    s.run("batch", "<ROOT>/b.json", "--dry-run")
    add("batch-dry-run-calls-nothing", "BatchTest::test_dry_run_calls_nothing", s)
    s = Scn().put("b.json", plan(steps))
    s.run("batch", "<ROOT>/b.json", "--write", "--out", "<ROOT>/out.json")
    add("batch-references-and-records", "BatchTest::test_references_and_records", s)
    s = Scn().put("b.json", plan([steps[0], {"name": "bad", "api": {"method": "POST", "path": "/fail", "data": {}}}, steps[1]]))
    s.run("batch", "<ROOT>/b.json", "--write", "--out", "<ROOT>/out.json", stderr_has=["stopped at bad"])
    add("batch-stops-at-first-failure", "BatchTest::test_stops_at_first_failure", s)
    s = Scn().put("b.json", plan([
        {"name": "doc", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues",
                                "data": {"title": "x", "body": "Use {{step.field}} or {{self.id}} here"}}},
        {"name": "later", "api": {"method": "POST", "path": "/repos/o/r/issues/{{doc.number}}/comments",
                                  "data": {"body": "refs {{doc.number}} and { { doc.id } } and {{ doc.id }}"}}}]))
    s.run("batch", "<ROOT>/b.json", "--write")
    add("batch-text-that-quotes-the-syntax-stays", "BatchTest::test_text_that_quotes_the_syntax_stays", s)
    s = Scn().put("b.json", plan([{"name": "list", "api": {"method": "GET", "path": "/repos/o/r/issues"}}]))
    s.run("batch", "<ROOT>/b.json")
    add("batch-reads-need-no-write", "BatchTest::test_reads_need_no_write", s)
    s = Scn().put("dup.json", plan([{"name": "a", "api": {"path": "/x"}}, {"name": "a", "api": {"path": "/y"}}]))
    s.put("noname.json", plan([{"api": {"path": "/x"}}]))
    s.put("badplat.json", json.dumps({"platform": "svn", "steps": []}))
    s.put("kind.json", plan([{"name": "a"}]))
    s.put("missing-ref.json", plan([{"name": "a", "api": {"method": "GET", "path": "/ok"}},
                                    {"name": "b", "api": {"method": "GET", "path": "/x/{{a.nope}}"}},
                                    {"name": "c", "api": {"method": "GET", "path": "/x"}}]))
    for f in ("dup", "noname", "badplat", "kind"):
        s.run("batch", f"<ROOT>/{f}.json", "--write")
    s.run("batch", "<ROOT>/missing-ref.json", "--out", "<ROOT>/out.json")
    add("batch-plan-errors", "reporun.cmd_batch (validation and reference errors)", s)
    s = Scn().put("b.json", plan([
        {"name": "list", "api": {"method": "GET", "path": "/repos/o/r/issues", "query": {"state": "open", "n": 2}}},
        {"name": "pages", "api": {"method": "GET", "path": "/page1", "paginate": True}}], "github"))
    s.run("batch", "<ROOT>/b.json", "--out", "<ROOT>/out.json")
    add("batch-query-and-paginate-steps", "reporun.cmd_batch (api options)", s)
    s = Scn().tool("gh", [{"args": "api repos/o/r/pulls", "stdout": '[{"number": 7, "url": "https://x/7"}]\n'},
                          {"args": "pr view", "stdout": "plain text\n"}])
    s.put("b.json", plan([{"name": "prs", "cli": ["api", "repos/o/r/pulls"]},
                          {"name": "text", "cli": ["pr", "view", "{{prs.0.url}}"]}]))
    s.run("batch", "<ROOT>/b.json", "--out", "<ROOT>/out.json")
    add("batch-cli-steps-keep-json-structured", "reporun.cmd_batch (cli steps)", s)

    # ChainTest
    commit = ["commit", "-q", "--allow-empty", "-m", "init"]
    s = Scn(token=False, stub=False)
    s.git("checkout", "-q", "-b", "main").git(*commit)
    for b in ("dev", "homolog", "feat/x"):
        s.git("branch", "-f", b)
    s.run("chain", "--json").run("chain")
    add("chain-full", "ChainTest::test_full_chain", s)
    s = Scn(token=False, stub=False)
    s.git("checkout", "-q", "-b", "main").git(*commit)
    for b in ("dev", "develop", "master"):
        s.git("branch", "-f", b)
    s.run("chain", "--json").run("chain")
    add("chain-ambiguous-asks", "ChainTest::test_ambiguous_asks", s)
    s = Scn(token=False, stub=False)
    s.git("checkout", "-q", "-b", "main").git(*commit).git("branch", "-f", "feat/x")
    s.run("chain", "--json").run("chain")
    add("chain-no-integration-branch", "ChainTest::test_no_integration_branch", s)
    s = Scn(token=False, stub=False)
    s.git("checkout", "-q", "-b", "trunk").git(*commit)
    s.git("update-ref", "refs/remotes/origin/trunk", "HEAD").git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk")
    s.run("chain", "--json").run("chain")
    add("chain-remote-default-branch", "reporun.cmd_chain (no main or master)", s)
    s = Scn(token=False, stub=False)
    s.git("checkout", "-q", "-b", "trunk").git(*commit)
    s.run("chain", "--json")
    add("chain-no-production-and-no-default", "reporun.cmd_chain (nothing to go on)", s)

    # Canary
    s = Scn().tool("gh", [{"args": "pr list", "stdout": "leak {env:GH_TOKEN} {env:AZURE_DEVOPS_EXT_PAT}\n"}])
    s.run("cli", "github", "--", "pr", "list")
    s.run("api", "github", "GET", "/echo-token", stderr_has=["<token>"])
    s.put("b.json", plan([{"name": "x", "api": {"method": "GET", "path": "/echo-token"}}]))
    s.run("batch", "<ROOT>/b.json", "--out", "<ROOT>/out.json")
    add("canary-token-never-leaves-through-output", "FR-A5", s)
    # Process list (FR-A5): while the child runs and while curl waits, the token is in no command line.
    # The Rust runner scans /proc during this case ("scan": true); the oracle just runs it.
    s = Scn().tool("gh").env(FAKE_TOOL_SLEEP="1")
    s.c["scan"] = True
    s.run("cli", "github", "--", "pr", "list")
    s.run("api", "github", "GET", "/slow")
    add("canary-process-list", "FR-A5 (process list)", s)
    return out


# ----------------------------------------------------------------- materialise

def private_path(root, extra_dirs):
    """A PATH with only the fake tools, git and curl: a real gh or az on this machine never interferes."""
    bin_dir = root / "bin"
    for tool in ("git", "curl"):
        found = shutil.which(tool)
        if found:
            link = bin_dir / Path(found).name
            if not link.exists():
                try:
                    os.symlink(found, link)
                except OSError:
                    shutil.copy2(found, link)
    return str(bin_dir)


def install_tool(root, name, spec, fake, stub_url):
    dest = root / "bin" / name
    if fake.suffix == ".py":
        body = fake.read_text(encoding="utf-8").split("\n", 1)[1]
        dest.write_text(f"#!{sys.executable}\n{body}", encoding="utf-8")
        dest.chmod(0o755)
    else:
        shutil.copy2(fake, dest.with_suffix(".exe") if os.name == "nt" else dest)
    rules = {"rules": [{**r, "capture": r["capture"].replace("<ROOT>", str(root))} if r.get("capture") else r
                       for r in spec.get("rules", [])]}
    (root / "bin" / f"{name}.rules.json").write_text(json.dumps(rules), encoding="utf-8")


def git(root, args, env):
    r = subprocess.run(["git", *args], cwd=root / "repo", env=env, capture_output=True, text=True)
    if r.returncode:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.strip()}")


def materialise(case, root, fake, stub_url):
    for d in ("home", "store", "bin", "repo"):
        (root / d).mkdir(parents=True, exist_ok=True)
    private_path(root, [])
    env = base_env(root)
    r = subprocess.run(["git", "init", "-q", str(root / "repo")], env=env, capture_output=True, text=True)
    if r.returncode:
        raise RuntimeError(f"git init: {r.stderr.strip()}")
    if case["remote"]:
        git(root, ["remote", "add", "origin", case["remote"]], env)
    for name, spec in {"pass": {}, **case["tools"]}.items():
        install_tool(root, name, spec, fake, stub_url)
    for entry, text in case["store"].items():
        p = root / "store" / f"{entry}.gpg"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8", newline="")
    for rel, text in case["files"].items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text.replace("<ROOT>", str(root)), encoding="utf-8", newline="")
    for args in case["git"]:
        git(root, args, env)
    return env


def base_env(root):
    env = {"HOME": str(root / "home"), "PASSWORD_STORE_DIR": str(root / "store"), "PATH": str(root / "bin"),
           "GIT_CONFIG_GLOBAL": str(root / "gitconfig"), "GIT_CONFIG_NOSYSTEM": "1", "NO_COLOR": "1", "TZ": "UTC",
           "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t",
           "LANG": "C.UTF-8"}
    for k in ("SystemRoot", "TEMP", "TMP", "TMPDIR", "PATHEXT", "COMSPEC", "SYSTEMROOT"):
        if os.environ.get(k):
            env[k] = os.environ[k]
    return env


def tree(root):
    return {str(p.relative_to(root)).replace("\\", "/"): p.read_bytes() for p in sorted(root.rglob("*"))
            if p.is_file() and ".git" not in p.relative_to(root).parts}


def normalise(text, root, stub_url):
    text = text.replace(str(root), "<ROOT>")
    if stub_url:
        text = text.replace(stub_url.removeprefix("http://"), "<STUB>")
    return text


def execute(case, tool_cmd, prefix, fake):
    root = Path(tempfile.mkdtemp(prefix="parity-repo-"))
    server = None
    try:
        stub_url = ""
        if case["stub"]:
            handler = type("H", (Stub,), {"seen": [], "counter": 100})
            server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
            threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.05}, daemon=True).start()
            stub_url = f"http://127.0.0.1:{server.server_address[1]}"
        env = materialise(case, root, fake, stub_url)
        if case["stub"]:
            env["REPORUN_TEST_BASE"] = stub_url
        for k, v in case["env"].items():
            if v is None:
                env.pop(k, None)
            else:
                env[k] = v.replace("{stub}", stub_url)
        before = tree(root)
        runs, cursor = [], 0
        for run in case["runs"]:
            for args in run.get("git", []):
                git(root, args, env)
            argv = [a.replace("<ROOT>", str(root)) for a in run["argv"]]
            proc = subprocess.run([*tool_cmd, *prefix, *argv], cwd=root / "repo", env=env, input=run.get("stdin"),
                                  capture_output=True, text=True, timeout=120)
            seen = handler.seen[cursor:] if server else []
            cursor += len(seen)
            runs.append({"argv": run["argv"], "stdin": run.get("stdin"), "git": run.get("git"), "exit": proc.returncode,
                         "stdout": normalise(proc.stdout, root, stub_url),
                         "stderr": normalise(proc.stderr, root, stub_url),
                         "stderr_has": run.get("stderr_has", []),
                         "seen": [{**x, "path": normalise(x["path"], root, stub_url)} for x in seen]})
        files = {}
        for rel, data in tree(root).items():
            if rel.startswith(("bin/", "home/", "store/", "gitconfig")):
                continue
            if before.get(rel) != data:
                files[rel] = normalise(data.decode("utf-8"), root, stub_url)
        return runs, files
    finally:
        if server:
            server.shutdown()
            server.server_close()
        shutil.rmtree(root, ignore_errors=True)


def canary(name, runs, files):
    problems = []
    for i, r in enumerate(runs):
        if TOKEN in r["stdout"] or TOKEN in r["stderr"]:
            problems.append(f"{name} run {i}: the token reached stdout or stderr")
    for rel, text in files.items():
        if TOKEN in text:
            problems.append(f"{name}: the token reached {rel}")
    return problems


def compare(name, case, runs, files):
    problems = canary(name, runs, files)
    for i, (want, got) in enumerate(zip(case["runs"], runs)):
        label = " ".join(want["argv"][:3])
        if want.get("exit") != got["exit"]:
            problems.append(f"{name} run {i} ({label}): exit {got['exit']}, expected {want.get('exit')}")
        if want.get("stdout") != got["stdout"]:
            problems.append(f"{name} run {i} ({label}): stdout differs\n--- expected\n{want.get('stdout')}\n--- got\n{got['stdout']}")
        for needle in want.get("stderr_has", []):
            if needle not in got["stderr"]:
                problems.append(f"{name} run {i} ({label}): stderr lacks {needle!r}\n{got['stderr']}")
        if want.get("seen") != got["seen"]:
            problems.append(f"{name} run {i} ({label}): requests differ\n--- expected\n{json.dumps(want.get('seen'), indent=1)}\n--- got\n{json.dumps(got['seen'], indent=1)}")
    for rel in sorted(set(case["expect_files"]) | set(files)):
        if case["expect_files"].get(rel) != files.get(rel):
            problems.append(f"{name}: file {rel} differs\n--- expected\n{case['expect_files'].get(rel)}\n--- got\n{files.get(rel)}")
    return problems


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin")
    ap.add_argument("--fake-bin", help="the Rust fake_tool example to install as the fake tools (with --bin)")
    ap.add_argument("--bless", action="store_true")
    ap.add_argument("--only")
    a = ap.parse_args()
    if a.bin:
        tool, prefix = [str(Path(a.bin).resolve())], ["repo"]
        fake = Path(a.fake_bin).resolve() if a.fake_bin else FAKE_PY
    else:
        tool, prefix, fake = [sys.executable, str(PY_TOOL)], [], FAKE_PY
    if a.bless:
        if not PY_TOOL.exists():
            print("refusing to bless: plugins/repo/scripts/reporun.py is gone (the fixtures are the oracle now)")
            return 1
        FIXTURES.mkdir(parents=True, exist_ok=True)
        for old in FIXTURES.glob("*.json"):
            old.unlink()
        bad = []
        for name, (derived, scn) in cases().items():
            case = {"name": name, "derived_from": derived, **scn.c}
            runs, files = execute(case, tool, prefix, fake)
            case["runs"] = [{"argv": r["argv"], "stdin": r["stdin"], **({"git": r["git"]} if r["git"] else {}),
                             "exit": r["exit"], "stdout": r["stdout"], "stderr_has": r["stderr_has"], "seen": r["seen"]}
                            for r in runs]
            case["expect_files"] = files
            bad += compare(name, case, runs, files)
            (FIXTURES / f"{name}.json").write_text(json.dumps(case, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        for p in bad:
            print("BLESS PROBLEM", p)
        print(f"blessed {len(list(FIXTURES.glob('*.json')))} cases into {FIXTURES.relative_to(ROOT)}")
        return 1 if bad else 0
    problems, n = [], 0
    for path in sorted(FIXTURES.glob("*.json")):
        if a.only and a.only not in path.stem:
            continue
        case = json.loads(path.read_text(encoding="utf-8"))
        runs, files = execute(case, tool, prefix, fake)
        problems += compare(case["name"], case, runs, files)
        n += 1
    for p in problems:
        print("FAIL ", p)
    who = "binary" if a.bin else "python oracle"
    print(f"{'FAIL' if problems else 'ok'}    {n} parity cases ({who}), {len(problems)} difference(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
