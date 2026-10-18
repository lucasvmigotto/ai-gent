"""Unit tests for reporun: python3 -m unittest plugins/repo/scripts/test_reporun.py

Covers remote parsing for every platform, the read/write classification of
CLI calls and API requests, token entry rules, and end-to-end calls through a
fake `pass`, a fake `gh` and a local HTTP stub standing in for each API:
auth headers, pagination, the host lock and redaction.
"""

import base64
import contextlib
import http.server
import io
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import reporun  # noqa: E402

TOKEN = "tok-3f9a1c77e0"

FAKE_PASS = """#!/bin/sh
store="$PASSWORD_STORE_DIR"
case "$1" in
  show) [ -f "$store/$2.gpg" ] || { echo "Error: $2 is not in the password store." >&2; exit 1; }
        cat "$store/$2.gpg" ;;
  *) exit 2 ;;
esac
"""

# A gh that shows what it received: its arguments and whether the token reached it.
FAKE_GH = """#!/bin/sh
echo "args: $*"
echo "token: $GH_TOKEN"
"""


class RemoteTest(unittest.TestCase):
    def test_platforms(self):
        cases = {
            "git@github.com:o/r.git": ("github", {"owner": "o", "repo": "r"}),
            "https://github.com/o/r": ("github", {"owner": "o", "repo": "r"}),
            "https://acme.ghe.com/o/r.git": ("github", {"owner": "o", "repo": "r"}),
            "https://org@dev.azure.com/org/My%20Proj/_git/app": ("azure-devops", {"org": "org", "project": "My Proj", "repo": "app"}),
            "git@ssh.dev.azure.com:v3/org/proj/app": ("azure-devops", {"org": "org", "project": "proj", "repo": "app"}),
            "https://org.visualstudio.com/DefaultCollection/proj/_git/app": ("azure-devops", {"org": "org", "project": "proj", "repo": "app"}),
            "https://dev.azure.com/org/_git/app": ("azure-devops", {"org": "org", "project": "app", "repo": "app"}),
            "git@gitlab.com:grp/sub/proj.git": ("gitlab", {"namespace": "grp/sub", "repo": "proj", "project_path": "grp/sub/proj"}),
            "git@bitbucket.org:ws/app.git": ("bitbucket", {"workspace": "ws", "repo": "app"}),
            "https://git.example.com/a/b": ("", {"owner": "a", "repo": "b"}),
        }
        for url, (platform, facts) in cases.items():
            with self.subTest(url=url):
                info = reporun.parse_remote(url)
                self.assertEqual(info["platform"], platform)
                for k, v in facts.items():
                    self.assertEqual(info[k], v)

    def test_forced_platform(self):
        self.assertEqual(reporun.parse_remote("https://code.example.com/a/b", "gitlab")["project_path"], "a/b")


class ClassifyTest(unittest.TestCase):
    def test_cli(self):
        reads = [("gh", "pr list"), ("gh", "pr view 3 --comments"), ("gh", "pr checks 3"), ("gh", "issue list --search bug"),
                 ("gh", "api repos/o/r/pulls"), ("gh", "run watch 5"), ("az", "boards work-item show --id 3"),
                 ("az", "boards query --wiql x"), ("az", "repos pr list"), ("az", "devops invoke --area wit"),
                 ("glab", "mr view 2"), ("glab", "issue list"), ("jira", "issue list"), ("jira", "issue view X-1")]
        writes = [("gh", "pr create --title list"), ("gh", "issue comment 5 --body x"), ("gh", "pr merge 3 --squash"),
                  ("gh", "api repos/o/r/issues -f title=x"), ("gh", "api -X PATCH repos/o/r/issues/1"),
                  ("gh", "-R o/r pr list"), ("gh", "run rerun 5"), ("az", "boards work-item update --id 3 --state Done"),
                  ("az", "devops invoke --area wit --http-method PATCH"), ("glab", "mr merge 2"), ("jira", "issue move X-1 Done"),
                  # A read word in an argument position isn't the verb.
                  ("gh", "label create status"), ("gh", "issue edit 12 view"), ("gh", "pr comment 3 list"),
                  ("glab", "issue note 4 show"), ("jira", "issue comment add KEY-1 list")]
        for cli, words in reads:
            with self.subTest(read=words):
                self.assertFalse(reporun.cli_is_write(cli, words.split()))
        for cli, words in writes:
            with self.subTest(write=words):
                self.assertTrue(reporun.cli_is_write(cli, words.split()))

    def test_api(self):
        self.assertFalse(reporun.api_is_write("github", "GET", "/repos/o/r/issues", None))
        self.assertFalse(reporun.api_is_write("github", "POST", "/graphql", b'{"query":"query { viewer { login } }"}'))
        self.assertTrue(reporun.api_is_write("github", "POST", "/graphql", b'{"query":"mutation { x }"}'))
        self.assertFalse(reporun.api_is_write("azure-devops", "POST", "/proj/_apis/wit/wiql", b"{}"))
        self.assertFalse(reporun.api_is_write("jira", "POST", "/rest/api/3/search/jql", b"{}"))
        self.assertTrue(reporun.api_is_write("jira", "POST", "/rest/api/3/issue", b"{}"))
        self.assertTrue(reporun.api_is_write("bitbucket", "DELETE", "/repositories/w/r/pullrequests/1", None))


class Stub(http.server.BaseHTTPRequestHandler):
    seen: list = []
    counter = 100

    def log_message(self, *a):
        pass

    def reply(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        Stub.seen.append({"method": self.command, "path": self.path, "headers": dict(self.headers), "body": body})
        if self.path.startswith("/echo-token"):
            self.send_response(401)
            self.end_headers()
            self.wfile.write(f'{{"message": "bad credentials {TOKEN}"}}'.encode())
            return
        if self.command == "POST" and self.path.split("?")[0].endswith("/issues"):
            Stub.counter += 1
            payload = {"number": Stub.counter, "id": 9000 + Stub.counter,
                       "html_url": f"https://github.com/o/r/issues/{Stub.counter}"}
            self.send_response(201)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(payload).encode())
            return
        if self.path.startswith("/fail"):
            self.send_response(422)
            self.end_headers()
            self.wfile.write(b'{"message": "Validation Failed"}')
            return
        if self.path.startswith("/page1"):
            port = self.server.server_address[1]
            payload = {"values": [1, 2], "next": f"http://127.0.0.1:{port}/page2"}
        elif self.path.startswith("/page2"):
            payload = {"values": [3]}
        else:
            payload = {"ok": True}
        self.send_response(201 if self.command == "POST" else 200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(payload).encode())

    do_GET = do_POST = do_PATCH = do_DELETE = reply


class Sandbox(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.store = base / "store"
        self.store.mkdir()
        bin_dir = base / "bin"
        bin_dir.mkdir()
        for name, text in (("pass", FAKE_PASS), ("gh", FAKE_GH)):
            (bin_dir / name).write_text(text)
            (bin_dir / name).chmod(0o755)
        self.repo = base / "repo"
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        subprocess.run(["git", "-C", str(self.repo), "remote", "add", "origin", "git@github.com:o/r.git"], check=True)
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Stub)
        threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.05}, daemon=True).start()
        self.base_url = f"http://127.0.0.1:{self.server.server_address[1]}"
        env = {"PASSWORD_STORE_DIR": str(self.store), "PATH": f"{bin_dir}:{os.environ['PATH']}",
               "REPORUN_TEST_BASE": self.base_url, "AI_GENT_PASS_PREFIX": None,
               "GIT_CONFIG_GLOBAL": str(base / "gitconfig"), "JIRA_URL": None}
        self.old_env = {k: os.environ.get(k) for k in env}
        for k, v in env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.old_cwd = os.getcwd()
        os.chdir(self.repo)
        Stub.seen.clear()
        Stub.counter = 100
        reporun.SECRETS.clear()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        os.chdir(self.old_cwd)
        for k, v in self.old_env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()

    def entry(self, name, text):
        f = self.store / f"{name}.gpg"
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(text)

    def run_cmd(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = reporun.main(list(argv))
        return code, out.getvalue(), err.getvalue()


class EntryTest(Sandbox):
    def test_entry_rules(self):
        self.assertEqual(reporun.default_entry("azure-devops"), "ai-gent/azure-devops/pat")
        with self.assertRaises(reporun.Refused):
            reporun.check_entry("ai-gent/db/shop-dev")
        with self.assertRaises(reporun.Refused):
            reporun.check_entry("personal/bank")
        with self.assertRaises(reporun.Failed):
            reporun.check_entry("ai-gent/../x")

    def test_notes(self):
        self.entry("ai-gent/jira/token", f"{TOKEN}\nemail: me@example.com\nurl: https://acme.atlassian.net/\nscopes - read\n")
        secret, notes = reporun.read_entry("ai-gent/jira/token")
        self.assertEqual(secret, TOKEN)
        self.assertEqual(notes, {"email": "me@example.com", "url": "https://acme.atlassian.net/"})

    def test_detect_never_decrypts(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, out, _ = self.run_cmd("detect", "--json")
        self.assertEqual(code, 0)
        report = json.loads(out)
        self.assertEqual(report["repository"]["platform"], "github")
        self.assertTrue(report["platforms"]["github"]["token_in_pass"])
        self.assertNotIn(TOKEN, out)
        self.assertEqual(reporun.SECRETS, [])


class ApiTest(Sandbox):
    def test_github_bearer_and_placeholders(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, out, err = self.run_cmd("api", "github", "GET", "/repos/{owner}/{repo}/issues", "--query", "state=open")
        self.assertEqual(code, 0, err)
        req = Stub.seen[-1]
        self.assertEqual(req["path"], "/repos/o/r/issues?state=open")
        self.assertEqual(req["headers"]["Authorization"], f"Bearer {TOKEN}")
        self.assertNotIn(TOKEN, out + err)

    def test_write_needs_flag(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        data = Path(self.tmp.name) / "b.json"
        data.write_text('{"title": "x"}')
        code, _, err = self.run_cmd("api", "github", "POST", "/repos/o/r/issues", "--data", str(data))
        self.assertEqual(code, reporun.EXIT_REFUSED)
        self.assertEqual(Stub.seen, [])
        code, _, err = self.run_cmd("api", "github", "POST", "/repos/o/r/issues", "--data", str(data), "--write")
        self.assertEqual(code, 0, err)
        self.assertEqual(Stub.seen[-1]["body"], b'{"title": "x"}')

    def test_azure_basic_and_api_version(self):
        subprocess.run(["git", "remote", "set-url", "origin", "https://dev.azure.com/org/proj/_git/app"], check=True)
        self.entry("ai-gent/azure-devops/pat", f"{TOKEN}\n")
        code, _, err = self.run_cmd("api", "azure-devops", "GET", "/{project}/_apis/wit/workitems/12")
        self.assertEqual(code, 0, err)
        req = Stub.seen[-1]
        self.assertEqual(req["path"], "/proj/_apis/wit/workitems/12?api-version=7.1")
        self.assertEqual(req["headers"]["Authorization"], "Basic " + base64.b64encode(f":{TOKEN}".encode()).decode())

    def test_jira_basic_with_email(self):
        self.entry("ai-gent/jira/token", f"{TOKEN}\nemail: me@example.com\n")
        query = Path(self.tmp.name) / "q.json"
        query.write_text('{"jql": "project = KEY"}')
        code, _, err = self.run_cmd("api", "jira", "POST", "/rest/api/3/search/jql", "--data", str(query))
        self.assertEqual(code, 0, err)  # a search: a POST that only reads
        auth = Stub.seen[-1]["headers"]["Authorization"]
        self.assertEqual(auth, "Basic " + base64.b64encode(f"me@example.com:{TOKEN}".encode()).decode())

    def test_gitlab_project_path_encoded(self):
        subprocess.run(["git", "remote", "set-url", "origin", "git@gitlab.com:grp/sub/proj.git"], check=True)
        self.entry("ai-gent/gitlab/pat", f"{TOKEN}\n")
        code, _, err = self.run_cmd("api", "gitlab", "GET", "/projects/{project_path}/issues")
        self.assertEqual(code, 0, err)
        self.assertEqual(Stub.seen[-1]["path"], "/projects/grp%2Fsub%2Fproj/issues")

    def test_bitbucket_pagination(self):
        self.entry("ai-gent/bitbucket/token", f"{TOKEN}\n")
        code, out, err = self.run_cmd("api", "bitbucket", "GET", "/page1", "--paginate")
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out), [1, 2, 3])

    def test_other_hosts_refused(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, _, err = self.run_cmd("api", "github", "GET", "https://evil.example.com/x")
        self.assertEqual(code, reporun.EXIT_REFUSED)
        self.assertEqual(Stub.seen, [])

    def test_errors_redacted(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, out, err = self.run_cmd("api", "github", "GET", "/echo-token")
        self.assertEqual(code, reporun.EXIT_ERROR)
        self.assertIn("<token>", err)
        self.assertNotIn(TOKEN, out + err)

    def test_missing_entry(self):
        code, _, err = self.run_cmd("api", "gitlab", "GET", "/projects")
        self.assertEqual(code, reporun.EXIT_ERROR)
        self.assertIn("pass insert -m ai-gent/gitlab/pat", err)


class CliTest(Sandbox):
    def run_cli(self, *argv):
        # The CLI's output goes through sys.stdout from a subprocess pipe.
        return self.run_cmd("cli", *argv)

    def test_token_in_env_only_and_redacted(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, out, err = self.run_cli("github", "--", "pr", "list")
        self.assertEqual(code, 0, err)
        self.assertIn("args: pr list", out)
        self.assertIn("token: <token>", out)  # it reached gh, and its echo was redacted
        self.assertNotIn(TOKEN, out + err)

    def test_refusals(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        for argv in (["auth", "token"], ["alias", "set", "x", "!env"], ["pr", "list", "--debug"],
                     ["extension", "install", "x/y"], ["pr", "merge", "3"]):
            with self.subTest(argv=argv):
                code, out, _ = self.run_cli("github", "--", *argv)
                self.assertEqual(code, reporun.EXIT_REFUSED)
                self.assertEqual(out, "")

    def test_write_with_flag(self):
        code, out, err = self.run_cli("github", "--write", "--", "pr", "merge", "3")
        self.assertEqual(code, 0, err)
        self.assertIn("token from the CLI's own login", err)  # no entry: gh's own login


class BatchTest(Sandbox):
    def plan(self, steps):
        f = Path(self.tmp.name) / "batch.json"
        f.write_text(json.dumps({"platform": "github", "steps": steps}))
        return str(f)

    STEPS = [
        {"name": "feature", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues", "data": {"title": "[012] Export"}},
         "record": {"key": "feature", "ref": "#{{self.number}}", "id": "{{self.id}}", "url": "{{self.html_url}}", "state": "todo"}},
        {"name": "US1", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues", "data": {"title": "[012] US1: CSV"}},
         "record": {"key": "US1", "ref": "#{{self.number}}", "id": "{{self.id}}", "state": "todo"}},
        {"name": "US1-parent", "api": {"method": "POST", "path": "/repos/{owner}/{repo}/issues/{{feature.number}}/sub_issues",
                                       "data": {"sub_issue_id": "{{US1.id}}"}}},
    ]

    def test_needs_write_once(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, _, err = self.run_cmd("batch", self.plan(self.STEPS))
        self.assertEqual(code, reporun.EXIT_REFUSED)
        self.assertIn("3 of 3 steps", err)
        self.assertEqual(Stub.seen, [])

    def test_dry_run_calls_nothing(self):
        code, out, _ = self.run_cmd("batch", self.plan(self.STEPS), "--dry-run")
        self.assertEqual(code, 0)
        self.assertIn("US1-parent", out)
        self.assertEqual(Stub.seen, [])

    def test_references_and_records(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        code, out, err = self.run_cmd("batch", self.plan(self.STEPS), "--write")
        self.assertEqual(code, 0, err)
        report = json.loads(out)
        self.assertTrue(report["ok"])
        self.assertEqual(Stub.seen[-1]["path"], "/repos/o/r/issues/101/sub_issues")
        self.assertEqual(json.loads(Stub.seen[-1]["body"]), {"sub_issue_id": 9102})  # an int stays an int
        self.assertEqual(report["records"][0], {"key": "feature", "ref": "#101", "id": 9101,
                                                "url": "https://github.com/o/r/issues/101", "state": "todo"})

    def test_stops_at_first_failure(self):
        self.entry("ai-gent/github/pat", f"{TOKEN}\n")
        steps = [self.STEPS[0], {"name": "bad", "api": {"method": "POST", "path": "/fail", "data": {}}}, self.STEPS[1]]
        code, out, _ = self.run_cmd("batch", self.plan(steps), "--write")
        self.assertEqual(code, reporun.EXIT_ERROR)
        report = json.loads(out)
        self.assertFalse(report["ok"])
        self.assertEqual(report["skipped"], ["US1"])
        self.assertEqual(len(report["records"]), 1)  # what was created is still recorded

    def test_reads_need_no_write(self):
        code, out, err = self.run_cmd("batch", self.plan([{"name": "list", "api": {"method": "GET", "path": "/repos/o/r/issues"}}]))
        self.assertEqual(code, 0, err)


class ChainTest(Sandbox):
    def branches(self, *names):
        subprocess.run(["git", "commit", "-q", "--allow-empty", "-m", "init"], check=True,
                       env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
                            "GIT_COMMITTER_EMAIL": "t@t"})
        for n in names:
            subprocess.run(["git", "branch", "-f", n], check=True)

    def chain(self):
        code, out, _ = self.run_cmd("chain", "--json")
        self.assertEqual(code, 0)
        return json.loads(out)

    def test_full_chain(self):
        subprocess.run(["git", "checkout", "-q", "-b", "main"], check=True)
        self.branches("dev", "homolog", "feat/x")
        c = self.chain()
        self.assertEqual((c["integration"], c["staging"], c["production"]), ("dev", "homolog", "main"))
        self.assertEqual(c["integrated_on"], "dev")
        self.assertFalse(c["ask"])

    def test_ambiguous_asks(self):
        subprocess.run(["git", "checkout", "-q", "-b", "main"], check=True)
        self.branches("dev", "develop", "master")
        c = self.chain()
        self.assertTrue(c["ask"])
        self.assertIsNone(c["integration"])
        self.assertEqual(len(c["ambiguous"]), 2)

    def test_no_integration_branch(self):
        subprocess.run(["git", "checkout", "-q", "-b", "main"], check=True)
        self.branches("feat/x")
        self.assertEqual(self.chain()["integrated_on"], "main")


if __name__ == "__main__":
    unittest.main()
