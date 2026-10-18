#!/usr/bin/env python3
"""reporun — the repo plugin's path to GitHub, Azure DevOps, GitLab, Bitbucket
and Jira: issues, work items, pull requests and their checks.

Tokens come from pass (shared/secrets.md): `ai-gent/<platform>/<name>`. The
runner decrypts an entry only for the call that needs it, hands it to the
platform's own CLI through that process's environment, or builds the API's
auth header itself — never on a command line, never printed (output is
redacted). Writes need --write, which the guard turns into a question.

  reporun detect [--json]
  reporun cli <platform> [--write] -- <cli args...>
  reporun api <platform> METHOD PATH [--data FILE|-] [--query k=v ...] [--write]
                                     [--paginate] [--max-pages N]
  reporun batch FILE [--write] [--dry-run] [--out FILE]
  reporun chain [--json]

See plugins/repo/references/runner.md. Python 3.9+ stdlib only.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

EXIT_OK, EXIT_ERROR, EXIT_USAGE, EXIT_REFUSED = 0, 1, 2, 3


class Failed(Exception):
    def __init__(self, message: str, code: int = EXIT_ERROR):
        super().__init__(message)
        self.code = code


class Refused(Failed):
    def __init__(self, message: str):
        super().__init__(message, EXIT_REFUSED)


# ------------------------------------------------------------------ platforms

PLATFORMS = {
    "github": {"cli": "gh", "entry": "github/pat", "env": ("GH_TOKEN",)},
    "azure-devops": {"cli": "az", "entry": "azure-devops/pat", "env": ("AZURE_DEVOPS_EXT_PAT",)},
    "gitlab": {"cli": "glab", "entry": "gitlab/pat", "env": ("GITLAB_TOKEN",)},
    "bitbucket": {"cli": None, "entry": "bitbucket/token", "env": ()},
    "jira": {"cli": "jira", "entry": "jira/token", "env": ("JIRA_API_TOKEN",)},
}

# CLI words that only read. Anything else is a write and needs --write.
READ_VERBS = {
    "list", "ls", "view", "show", "status", "checks", "diff", "search", "query", "get",
    "watch", "download", "issues", "comments", "me", "serverinfo", "help", "--help", "version", "--version",
}
# Subcommands that print a token, run arbitrary code or log auth headers.
BLOCKED_CLI = {
    "gh": {"auth", "alias", "extension", "ext", "secret", "codespace", "config"},
    "glab": {"auth", "alias", "config", "variable", "token"},
    "az": {"login", "logout", "account", "extension", "config"},
    "jira": {"init"},
}
BLOCKED_FLAGS = {"--debug", "--show-token"}

# POSTs that only read (query languages over POST).
READ_POSTS = {
    "github": re.compile(r"^/graphql$"),
    "gitlab": re.compile(r"^/api/graphql$|^/graphql$"),
    "azure-devops": re.compile(r"/_apis/wit/wiql(/|$|\?)"),
    "jira": re.compile(r"^/rest/api/\d+/search(/jql)?$|^/rest/agile/[\d.]+/.*/search$"),
    "bitbucket": None,
}


# ----------------------------------------------------------------------- pass

def _store():
    """shared/passstore.py, linked next to this script."""
    import passstore  # noqa: PLC0415
    return passstore


def pass_prefix() -> str:
    return _store().prefix()


def default_entry(platform: str) -> str:
    return f"{pass_prefix()}/{PLATFORMS[platform]['entry']}"


def check_entry(entry: str) -> str:
    """A platform token entry: under the prefix, never a db/ entry, which is dbrun's."""
    try:
        name = _store().check(entry)
    except _store().StoreError as e:
        raise Failed(str(e)) from None
    if not name.startswith(pass_prefix() + "/") or name.startswith(f"{pass_prefix()}/db/"):
        raise Refused(f"{entry} isn't a platform token entry: use one under {pass_prefix()}/<platform>/ (db/ is dbrun's)")
    return entry


def pass_has(entry: str) -> bool:
    """Whether the store holds the entry: a file check, nothing decrypted."""
    try:
        return _store().has(entry)
    except _store().StoreError:
        return False


def read_entry(entry: str, timeout: float = 90) -> tuple[str, dict[str, str]]:
    """The secret (first line) and `key: value` notes from later lines."""
    check_entry(entry)
    try:
        secret, rest = _store().read(entry, timeout)
    except _store().StoreError as e:
        raise Failed(f"{e}; see secrets:pass") from None
    notes = {}
    for line in rest:
        key, sep, value = line.partition(":")
        if sep and re.fullmatch(r"[A-Za-z][A-Za-z0-9_-]*", key.strip()):
            notes[key.strip().lower()] = value.strip()
    SECRETS.append(secret)
    return secret, notes


# ------------------------------------------------------------------ redaction

SECRETS: list[str] = []


def redact(text: str) -> str:
    for s in SECRETS:
        if len(s) >= 4:
            text = text.replace(s, "<token>")
            text = text.replace(base64.b64encode(f":{s}".encode()).decode(), "<token>")
    return text


# -------------------------------------------------------------------- detect


def git(*args: str) -> str:
    try:
        out = subprocess.run(["git", *args], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return ""
    return out.stdout.strip() if out.returncode == 0 else ""


def git_config(key: str) -> str:
    return git("config", "--get", key)


def parse_remote(url: str, forced: str = "") -> dict:
    """Platform, host and path parts from a remote URL (https, ssh or scp-like)."""
    url = url.strip()
    m = re.match(r"^[\w.+-]+@([^:/]+):(.*)$", url)  # scp-like git@host:path
    if m and "://" not in url:
        host, path = m.group(1), m.group(2)
    else:
        u = urllib.parse.urlparse(url)
        host, path = (u.hostname or ""), u.path
    host = host.lower()
    parts = [urllib.parse.unquote(p) for p in path.strip("/").removesuffix(".git").split("/") if p]
    info: dict = {"url": url, "host": host, "platform": forced or ""}

    if host in ("dev.azure.com", "ssh.dev.azure.com") or host.endswith(".visualstudio.com"):
        info["platform"] = "azure-devops"
        if host == "ssh.dev.azure.com":  # v3/org/project/repo
            parts = parts[1:] if parts[:1] == ["v3"] else parts
            info["org"], info["project"], info["repo"] = (parts + ["", "", ""])[:3]
        else:
            if host == "dev.azure.com":
                info["org"], parts = (parts[0] if parts else ""), parts[1:]
            else:
                info["org"] = host.split(".")[0]
            if parts[:1] == ["DefaultCollection"]:
                parts = parts[1:]
            if "_git" in parts:  # [project/]_git/repo; no project means it's named like the repo
                i = parts.index("_git")
                info["repo"] = parts[i + 1] if i + 1 < len(parts) else ""
                info["project"] = parts[i - 1] if i else info["repo"]
        info["host"] = "dev.azure.com"
        return info
    if not info["platform"]:
        if host == "github.com" or host.endswith(".ghe.com"):
            info["platform"] = "github"
        elif host == "bitbucket.org":
            info["platform"] = "bitbucket"
        elif "gitlab" in host:
            info["platform"] = "gitlab"
    if info["platform"] == "gitlab":
        info["namespace"], info["repo"] = "/".join(parts[:-1]), (parts[-1] if parts else "")
        info["project_path"] = "/".join(parts)
    elif info["platform"] == "bitbucket":
        info["workspace"], info["repo"] = (parts + ["", ""])[:2]
    elif parts:
        info["owner"], info["repo"] = (parts + ["", ""])[:2]
    return info


def detect_repo() -> dict:
    remote = git("remote", "get-url", os.environ.get("AI_GENT_REPO_REMOTE", "origin"))
    if not remote:
        return {"platform": "", "url": ""}
    return parse_remote(remote, git_config("ai-gent.platform"))


def jira_url(notes: dict | None = None) -> str:
    return (git_config("ai-gent.jira.url") or os.environ.get("JIRA_URL") or (notes or {}).get("url", "")).rstrip("/")


def cli_state(platform: str) -> dict:
    name = PLATFORMS[platform]["cli"]
    if not name:
        return {"name": None, "installed": False}
    state: dict = {"name": name, "installed": bool(shutil.which(name))}
    if state["installed"] and platform in ("github", "gitlab"):
        r = subprocess.run([name, "auth", "status"], capture_output=True, text=True, timeout=20,
                           env={k: v for k, v in os.environ.items() if k not in ("GH_TOKEN", "GITHUB_TOKEN", "GITLAB_TOKEN")})
        state["signed_in"] = r.returncode == 0
    if state["installed"] and platform == "azure-devops":
        r = subprocess.run(["az", "extension", "show", "--name", "azure-devops", "-o", "none"],
                           capture_output=True, text=True, timeout=30)
        state["devops_extension"] = r.returncode == 0
    return state


def cmd_detect(args) -> int:
    repo = detect_repo()
    report = {"repository": repo, "platforms": {}}
    wanted = [repo["platform"]] if repo.get("platform") else []
    if jira_url() or pass_has(default_entry("jira")):
        wanted.append("jira")
    for platform in wanted:
        entry = default_entry(platform)
        report["platforms"][platform] = {"token_entry": entry, "token_in_pass": pass_has(entry), "cli": cli_state(platform)}
        if platform == "jira":
            report["platforms"][platform]["url"] = jira_url() or "(set git config ai-gent.jira.url, or url: in the entry)"
    if args.json:
        print(json.dumps(report, indent=2))
        return EXIT_OK
    print(f"remote    {repo.get('url') or '(none)'}")
    print(f"platform  {repo.get('platform') or 'unknown — set git config ai-gent.platform <github|gitlab|azure-devops|bitbucket>'}")
    for k in ("owner", "org", "project", "namespace", "workspace", "repo"):
        if repo.get(k):
            print(f"{k:<9} {repo[k]}")
    for platform, p in report["platforms"].items():
        cli = p["cli"]
        how = "CLI" if cli.get("installed") else "REST API"
        extra = ""
        if "signed_in" in cli:
            extra = ", signed in" if cli["signed_in"] else ", not signed in"
        if "devops_extension" in cli:
            extra = ", azure-devops extension" + ("" if cli["devops_extension"] else " MISSING (az extension add --name azure-devops)")
        token = "in pass" if p["token_in_pass"] else "not in pass"
        print(f"\n{platform}: {how} ({cli.get('name') or 'no CLI'}{' installed' if cli.get('installed') else ''}{extra})")
        print(f"  token   {p['token_entry']} — {token}")
        if platform == "jira":
            print(f"  url     {p['url']}")
        if not p["token_in_pass"]:
            print(f"  add it in your own terminal: pass insert -m {p['token_entry']}")
    return EXIT_OK


# ----------------------------------------------------------------------- cli


def cli_is_write(cli: str, words: list[str]) -> bool:
    """Whether a CLI call changes something. Unknown = write, and so are options
    before the subcommand (`gh -R o/r pr merge`): put them after it."""
    if words and words[0].startswith("-") and words[0] not in ("-h", "--help", "--version"):
        return True
    positional = []
    for w in words:
        if w.startswith("-"):
            break
        positional.append(w)
    if not positional:
        return False  # bare `gh`, `gh --help`
    if positional[0] == "api" and cli in ("gh", "glab"):
        method = "GET"
        for i, w in enumerate(words):
            if w in ("-X", "--method") and i + 1 < len(words):
                method = words[i + 1].upper()
            elif w.startswith("--method="):
                method = w.split("=", 1)[1].upper()
            elif w in ("-f", "-F", "--field", "--raw-field", "--input") or w.startswith(("--field=", "--raw-field=", "--input=")):
                method = "POST" if method == "GET" else method
        return method not in ("GET", "HEAD")
    if cli == "az" and positional[:2] == ["devops", "invoke"]:
        for i, w in enumerate(words):
            if w == "--http-method" and i + 1 < len(words):
                return words[i + 1].upper() not in ("GET", "HEAD")
            if w == "--in-file":
                return True
        return False
    if cli == "az" and positional[:2] == ["boards", "query"]:
        return False
    # The verb sits at a fixed place: gh, glab and jira are `<noun> <verb>`
    # (a lone word is the verb: `jira me`); az is `<group…> <verb>`, the last
    # word before the options. Words after the verb are arguments, never verbs
    # (`gh issue edit 12 view` edits).
    verb = positional[-1] if cli == "az" else (positional[1] if len(positional) > 1 else positional[0])
    return verb not in READ_VERBS


def prepare_cli(platform: str, words: list[str], entry_arg: str | None, write: bool) -> tuple[list[str], dict]:
    """The checked command line and its environment (token from pass, or the CLI's own login)."""
    cli = PLATFORMS[platform]["cli"]
    if not cli:
        raise Failed(f"{platform} has no CLI; use `reporun api {platform} ...`", EXIT_USAGE)
    if words and words[0] in (cli, Path(cli).name):
        words = words[1:]
    if not shutil.which(cli):
        raise Failed(f"{cli} isn't installed; use `reporun api {platform} ...` (the REST API) instead", EXIT_USAGE)
    positional = [w for w in words if not w.startswith("-")][:2]
    if positional and positional[0] in BLOCKED_CLI.get(cli, set()):
        raise Refused(f"`{cli} {positional[0]}` can print a token, run arbitrary code or change the CLI's own login; not through reporun")
    if any(w in BLOCKED_FLAGS or w.startswith("--debug") for w in words):
        raise Refused("debug and token-printing flags can log the auth header; drop them")
    if cli_is_write(cli, words) and not write:
        raise Refused(f"`{cli} {' '.join(words[:3])}` changes something on {platform}; only when the user asked for "
                      f"this specific change, re-run with --write")

    env = {k: v for k, v in os.environ.items() if k not in ("GH_DEBUG", "GLAB_DEBUG", "DEBUG")}
    entry = check_entry(entry_arg) if entry_arg else default_entry(platform)
    source = "the CLI's own login"
    if pass_has(entry):
        secret, notes = read_entry(entry)
        for var in PLATFORMS[platform]["env"]:
            env[var] = secret
        repo = detect_repo()
        if platform == "github" and repo.get("host") and repo["host"] not in ("github.com",) and not repo["host"].endswith(".ghe.com"):
            env["GH_ENTERPRISE_TOKEN"] = secret
        if platform == "gitlab" and repo.get("host") and repo["host"] != "gitlab.com":
            env.setdefault("GITLAB_HOST", repo["host"])
        source = f"pass ({entry})"
    elif entry_arg:
        raise Failed(f"{entry} isn't in pass; add it in your own terminal: pass insert -m {entry}")
    print(f"reporun: {cli} {' '.join(words[:3])} · token from {source}", file=sys.stderr)
    return [cli, *words], env


def run_cli_capture(platform: str, words: list[str], entry_arg: str | None, write: bool) -> str:
    argv, env = prepare_cli(platform, words, entry_arg, write)
    out = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, text=True, env=env, timeout=300)
    text = redact(out.stdout + out.stderr)
    if out.returncode != 0:
        raise Failed(f"{argv[0]} exited {out.returncode}: {text.strip()[-2000:]}")
    return redact(out.stdout).strip()


def cmd_cli(args) -> int:
    words = list(args.args)
    # reporun's own options may come after the platform, before `--`.
    while words and words[0] != "--" and words[0] in ("--write", "--entry"):
        if words[0] == "--write":
            args.write, words = True, words[1:]
        elif len(words) > 1:
            args.entry, words = words[1], words[2:]
        else:
            raise Failed("--entry needs a value", EXIT_USAGE)
    if words and words[0] == "--":
        words = words[1:]
    argv, env = prepare_cli(args.platform, words, args.entry, args.write)
    proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, env=env, bufsize=1)
    assert proc.stdout is not None
    for line in proc.stdout:
        sys.stdout.write(redact(line))
        sys.stdout.flush()
    return proc.wait()


# ----------------------------------------------------------------------- api


def api_base(platform: str, notes: dict) -> tuple[str, dict]:
    """Base URL and the repository facts used to fill {placeholders}."""
    override = os.environ.get("REPORUN_TEST_BASE")  # tests only: a local stub server
    repo = detect_repo()
    facts = {k: v for k, v in repo.items() if isinstance(v, str)}
    if platform == "jira":
        url = jira_url(notes)
        if not url and not override:
            raise Failed("no Jira URL: git config --global ai-gent.jira.url https://<site>.atlassian.net "
                         "(or a `url:` line in the token entry)")
        return override or url, facts
    if repo.get("platform") and repo["platform"] != platform:
        facts = {}
    if override:
        return override, facts
    host = repo.get("host", "") if facts else ""
    if platform == "github":
        return ("https://api.github.com" if host in ("", "github.com") else
                f"https://api.{host}" if host.endswith(".ghe.com") else f"https://{host}/api/v3"), facts
    if platform == "gitlab":
        return f"https://{host or 'gitlab.com'}/api/v4", facts
    if platform == "azure-devops":
        org = facts.get("org") or notes.get("org") or git_config("ai-gent.azure-devops.org")
        if not org:
            raise Failed("no Azure DevOps organization: not in the remote; set git config ai-gent.azure-devops.org <org>")
        return f"https://dev.azure.com/{org}", facts
    return "https://api.bitbucket.org/2.0", facts


def auth_header(platform: str, secret: str, notes: dict) -> str:
    user = notes.get("email") or notes.get("username") or notes.get("user")
    if platform == "azure-devops":
        return "Basic " + base64.b64encode(f":{secret}".encode()).decode()
    if user and platform in ("bitbucket", "jira"):
        return "Basic " + base64.b64encode(f"{user}:{secret}".encode()).decode()
    return "Bearer " + secret


def fill(path: str, facts: dict) -> str:
    def sub(m):
        key = m.group(1)
        if key not in facts or not facts[key]:
            raise Failed(f"{{{key}}} isn't known for this repository; write it out")
        return urllib.parse.quote(facts[key], safe="")  # GitLab wants group%2Fproject
    return re.sub(r"\{([a-z_]+)\}", sub, path)


def api_is_write(platform: str, method: str, path: str, body: bytes | None) -> bool:
    if method in ("GET", "HEAD"):
        return False
    rp = READ_POSTS.get(platform)
    if method == "POST" and rp and rp.search(urllib.parse.urlparse(path).path):
        return platform in ("github", "gitlab") and body is not None and b"mutation" in body
    return True


def request(method: str, url: str, headers: dict, body: bytes | None, timeout: float):
    req = urllib.request.Request(url, data=body, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers or {}), e.read()
    except urllib.error.URLError as e:
        raise Failed(f"{method} {redact(url)}: {e.reason}")


def next_link(platform: str, headers: dict, data) -> str:
    link = headers.get("Link") or headers.get("link") or ""
    m = re.search(r'<([^>]+)>;\s*rel="next"', link)
    if m:
        return m.group(1)
    if platform == "bitbucket" and isinstance(data, dict) and data.get("next"):
        return data["next"]
    return ""


class HttpError(Failed):
    def __init__(self, status: int, text: str):
        super().__init__(f"HTTP {status}: {text[:4000]}")
        self.status = status


def api_call(platform: str, method: str, path: str, body: bytes | None = None, query: dict | None = None,
             entry_arg: str | None = None, write: bool = False, paginate: bool = False, max_pages: int = 10,
             timeout: float = 30):
    """One REST request (or a paginated series) with the token from pass; the parsed JSON."""
    method = method.upper()
    if method not in ("GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"):
        raise Failed(f"method {method} isn't supported", EXIT_USAGE)
    if api_is_write(platform, method, path, body) and not write:
        raise Refused(f"{method} {path} changes something on {platform}; only when the user asked for this "
                      f"specific change, re-run with --write")

    entry = check_entry(entry_arg) if entry_arg else default_entry(platform)
    secret, notes = ("", {})
    if pass_has(entry):
        secret, notes = read_entry(entry)
    elif platform != "github" or entry_arg:
        raise Failed(f"{entry} isn't in pass; add it in your own terminal: pass insert -m {entry}")
    base, facts = api_base(platform, notes)
    path = fill(path, facts)
    if path.startswith(("http://", "https://")):
        if urllib.parse.urlparse(path).netloc != urllib.parse.urlparse(base).netloc:
            raise Refused(f"{urllib.parse.urlparse(path).netloc} isn't {platform}'s API host; the token goes nowhere else")
        url = path
    else:
        url = base.rstrip("/") + "/" + path.lstrip("/")
    query = dict(query or {})
    if platform == "azure-devops" and "api-version" not in url and "api-version" not in query:
        query["api-version"] = "7.1"
    if query:
        url += ("&" if "?" in url else "?") + urllib.parse.urlencode(query)

    headers = {"Accept": "application/json", "User-Agent": "ai-gent-reporun"}
    if platform == "github":
        headers["Accept"] = "application/vnd.github+json"
        headers["X-GitHub-Api-Version"] = "2022-11-28"
    if secret:
        headers["Authorization"] = auth_header(platform, secret, notes)
    if body is not None:
        headers["Content-Type"] = ("application/json-patch+json"
                                   if platform == "azure-devops" and method == "PATCH" and "/wit/workitems" in url.lower()
                                   or platform == "azure-devops" and method == "POST" and "/wit/workitems/$" in urllib.parse.unquote(url).lower()
                                   else "application/json")
    print(f"reporun: {method} {redact(url)} · token from {'pass (' + entry + ')' if secret else 'none (public)'}",
          file=sys.stderr)

    pages, results = 0, []
    while url and pages < max_pages:
        status, rh, raw = request(method, url, headers, body, timeout)
        pages += 1
        text = redact(raw.decode("utf-8", "replace"))
        if status >= 400:
            raise HttpError(status, text)
        try:
            data = json.loads(text) if text.strip() else None
        except json.JSONDecodeError:
            data = text
        if not paginate:
            return data
        results.append(data)
        nxt = next_link(platform, rh, data)
        if nxt and urllib.parse.urlparse(nxt).netloc != urllib.parse.urlparse(url).netloc:
            break
        url = nxt
    merged: list = []
    for page in results:
        if isinstance(page, list):
            merged.extend(page)
        elif isinstance(page, dict) and isinstance(page.get("values"), list):
            merged.extend(page["values"])
        elif isinstance(page, dict) and isinstance(page.get("value"), list):
            merged.extend(page["value"])
        else:
            merged.append(page)
    if url and pages >= max_pages:
        print(f"reporun: stopped after {pages} pages (--max-pages)", file=sys.stderr)
    return merged


def cmd_api(args) -> int:
    body = None
    if args.data:
        body = (sys.stdin.read() if args.data == "-" else Path(args.data).read_text()).encode()
    query = dict(q.split("=", 1) for q in args.query or [] if "=" in q)
    try:
        data = api_call(args.platform, args.method, args.path, body, query, args.entry, args.write,
                        args.paginate, args.max_pages, args.timeout)
    except HttpError as e:
        print(redact(str(e)), file=sys.stderr)
        return EXIT_ERROR
    if data is not None:
        print(json.dumps(data, indent=2, ensure_ascii=False) if not isinstance(data, str) else data)
    return EXIT_OK


# --------------------------------------------------------------------- batch

REF_RE = re.compile(r"\{\{\s*([A-Za-z0-9_.-]+)\s*\}\}")


def lookup(results: dict, dotted: str):
    name, _, rest = dotted.partition(".")
    if name not in results:
        raise Failed(f"{{{{{dotted}}}}}: no earlier step named {name!r}")
    value = results[name]
    for part in rest.split(".") if rest else []:
        if isinstance(value, list) and part.isdigit():
            value = value[int(part)]
        elif isinstance(value, dict) and part in value:
            value = value[part]
        else:
            raise Failed(f"{{{{{dotted}}}}}: {part!r} isn't in that step's result")
    return value


def resolve(value, results: dict, names: set[str] | None = None):
    """Fill {{step.field}} from earlier results; a string that is only a reference keeps its type.
    Only the batch's own step names (and `self`) are references: other {{…}} text — an issue
    body quoting the syntax, say — stays as written."""
    names = set(results) if names is None else names

    def ref(m):
        return m.group(1).partition(".")[0] in names

    if isinstance(value, str):
        m = REF_RE.fullmatch(value.strip())
        if m and ref(m):
            return lookup(results, m.group(1))
        return REF_RE.sub(lambda m: str(lookup(results, m.group(1))) if ref(m) else m.group(0), value)
    if isinstance(value, list):
        return [resolve(v, results, names) for v in value]
    if isinstance(value, dict):
        return {k: resolve(v, results, names) for k, v in value.items()}
    return value


def step_is_write(platform: str, step: dict) -> bool:
    if "api" in step:
        a = step["api"]
        body = json.dumps(a["data"]).encode() if "data" in a else None
        return api_is_write(platform, a.get("method", "GET").upper(), a.get("path", ""), body)
    if "cli" in step:
        return cli_is_write(PLATFORMS[platform]["cli"] or "", list(step["cli"]))
    raise Failed(f"step {step.get('name')!r} has neither api nor cli")


def cmd_batch(args) -> int:
    """Several calls, one confirmation: steps run in order and stop at the first failure."""
    plan = json.loads(Path(args.file).read_text())
    platform = plan.get("platform", "")
    if platform not in PLATFORMS:
        raise Failed(f"the batch's platform {platform!r} isn't one of {', '.join(PLATFORMS)}", EXIT_USAGE)
    steps = plan.get("steps") or []
    names = [s.get("name") for s in steps]
    if any(not n for n in names) or len(set(names)) != len(names):
        raise Failed("every step needs a unique name", EXIT_USAGE)
    writes = [s["name"] for s in steps if step_is_write(platform, s)]
    if writes and not args.write and not args.dry_run:
        raise Refused(f"{len(writes)} of {len(steps)} steps change something on {platform} ({', '.join(writes[:8])}"
                      f"{'…' if len(writes) > 8 else ''}); after the user confirmed this batch, re-run with --write")
    if args.dry_run:
        for s in steps:
            print(f"{s['name']:<16} {'write' if s['name'] in writes else 'read ':<5} "
                  + (f"{s['api'].get('method', 'GET')} {s['api'].get('path')}" if "api" in s else
                     " ".join(w.replace("\n", " ")[:48] + ("…" if len(w) > 48 else "") for w in s["cli"])))
        return EXIT_OK

    results: dict = {}
    refs = set(names) | {"self"}
    report: dict = {"platform": platform, "ok": True, "steps": [], "records": []}
    for s in steps:
        name = s["name"]
        try:
            if "api" in s:
                a = resolve(s["api"], results, set(names))
                body = json.dumps(a["data"]).encode() if "data" in a else None
                data = api_call(platform, a.get("method", "GET"), a["path"], body, a.get("query"),
                                args.entry, args.write, a.get("paginate", False), a.get("max_pages", 10), args.timeout)
            else:
                text = run_cli_capture(platform, resolve(list(s["cli"]), results, set(names)), args.entry, args.write)
                try:  # `gh api`, `glab api`, `az … -o json` print JSON: keep it structured
                    data = json.loads(text) if text else {}
                except json.JSONDecodeError:
                    data = {"stdout": text}
            results[name] = data if data is not None else {}
            results["self"] = results[name]
            if s.get("record"):
                report["records"].append(resolve(s["record"], results, refs))
            report["steps"].append({"name": name, "ok": True})
        except Failed as e:
            report["ok"] = False
            report["steps"].append({"name": name, "ok": False, "error": redact(str(e))[:2000]})
            break
        finally:
            results.pop("self", None)
    done = len([x for x in report["steps"] if x["ok"]])
    report["skipped"] = [s["name"] for s in steps[len(report["steps"]):]]
    out = json.dumps(report, indent=2, ensure_ascii=False)
    if args.out:
        Path(args.out).write_text(out + "\n")
    print(out)
    print(f"reporun: {done}/{len(steps)} steps done" + ("" if report["ok"] else f"; stopped at {report['steps'][-1]['name']}"),
          file=sys.stderr)
    return EXIT_OK if report["ok"] else EXIT_ERROR


# --------------------------------------------------------------------- chain

LEVELS = {"integration": ("dev", "develop"), "staging": ("staging", "homolog", "homologation", "hml"),
          "production": ("main", "master")}


def cmd_chain(args) -> int:
    """The branch chain from the branches that exist: work → integration → [staging] → production."""
    remote = os.environ.get("AI_GENT_REPO_REMOTE", "origin")
    local = set(git("for-each-ref", "--format=%(refname:short)", "refs/heads").split())
    remote_names = {r.split("/", 1)[1] for r in git("for-each-ref", "--format=%(refname:short)", f"refs/remotes/{remote}").split()
                    if "/" in r and not r.endswith("/HEAD")}
    names = local | remote_names
    default = git("symbolic-ref", "--short", f"refs/remotes/{remote}/HEAD").removeprefix(f"{remote}/")
    chain: dict = {"default": default or None, "ambiguous": []}
    for level, candidates in LEVELS.items():
        found = [c for c in candidates if c in names]
        chain[level] = found[0] if len(found) == 1 else None
        if len(found) > 1:
            chain["ambiguous"].append(f"{level}: {' and '.join(found)} both exist")
    if not chain["production"] and not any(c in names for c in LEVELS["production"]):
        chain["production"] = default or None
        if not default:
            chain["ambiguous"].append("production: no main/master and no remote default branch")
    chain["order"] = [x for x in ("work branches", chain["integration"], chain["staging"], chain["production"]) if x]
    # Where a story's work counts as integrated, and where closing keywords act.
    chain["integrated_on"] = chain["integration"] or chain["production"]
    chain["keywords_close_on"] = default or chain["production"]
    chain["ask"] = bool(chain["ambiguous"])
    if args.json:
        print(json.dumps(chain, indent=2))
        return EXIT_OK
    print("chain     " + " → ".join(chain["order"]))
    print(f"default   {chain['default'] or '(unknown — no remote HEAD)'}")
    print(f"integrated on   {chain['integrated_on'] or '?'}   (where a story's work lands; repo:sync closes items at Verified there)")
    print(f"keywords close  {chain['keywords_close_on'] or '?'}   (Closes #N acts only when merged here)")
    for a in chain["ambiguous"]:
        print(f"ASK       {a} — ask the user which one the flow uses")
    return EXIT_OK


# --------------------------------------------------------------------- main


def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(prog="reporun", description="The repo plugin's path to code platforms (references/runner.md).")
    sub = ap.add_subparsers(dest="command", required=True)

    s = sub.add_parser("detect", help="platform, CLI and token state for this repository (nothing decrypted)")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_detect)

    s = sub.add_parser("cli", help="run the platform's own CLI with the token from pass")
    s.add_argument("platform", choices=[p for p, v in PLATFORMS.items() if v["cli"]])
    s.add_argument("--write", action="store_true", help="the user asked for this specific change")
    s.add_argument("--entry", help="another pass entry under the prefix (default ai-gent/<platform>/...)")
    s.add_argument("args", nargs=argparse.REMAINDER)
    s.set_defaults(func=cmd_cli)

    s = sub.add_parser("batch", help="several calls from a JSON plan, one confirmation (references/sync.md)")
    s.add_argument("file")
    s.add_argument("--write", action="store_true", help="the user confirmed this batch")
    s.add_argument("--dry-run", action="store_true", help="list the steps, call nothing")
    s.add_argument("--entry")
    s.add_argument("--out", help="also write the result JSON here")
    s.add_argument("--timeout", type=float, default=30)
    s.set_defaults(func=cmd_batch)

    s = sub.add_parser("chain", help="the branch chain: work → integration → [staging] → production")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_chain)

    s = sub.add_parser("api", help="call the platform's REST API with the token from pass")
    s.add_argument("platform", choices=list(PLATFORMS))
    s.add_argument("method")
    s.add_argument("path", help="relative to the API base; {owner} {repo} {org} {project} {workspace} {project_path} filled from the remote")
    s.add_argument("--data", help="request body file, or - for stdin")
    s.add_argument("--query", action="append", metavar="K=V")
    s.add_argument("--write", action="store_true", help="the user asked for this specific change")
    s.add_argument("--entry")
    s.add_argument("--paginate", action="store_true")
    s.add_argument("--max-pages", type=int, default=10)
    s.add_argument("--timeout", type=float, default=30)
    s.set_defaults(func=cmd_api)
    return ap


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except Refused as e:
        print(redact(f"REFUSED: {e}"), file=sys.stderr)
        return EXIT_REFUSED
    except Failed as e:
        print(redact(f"error: {e}"), file=sys.stderr)
        return e.code
    except KeyboardInterrupt:
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
