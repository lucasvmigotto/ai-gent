"""Language table for compact.py (`fmt` lists formatters; unused in this read-only copy).

Each language declares which formatting-removal transforms are *safe* for it:

  blank   drop blank lines and trailing whitespace
  indent  drop leading indentation, collapse runs of spaces inside a line to one
  join    remove newlines between tokens (per-language rules, see `join`)
  spaces  remove intra-line spaces that the lexer does not need

`join` selects the newline-joining rule:
  free    newlines are plain whitespace (Java, C, C#, Rust, PHP, JSON, ...)
  go      Go: re-insert the semicolons the Go lexer would insert
  js      JavaScript/TypeScript: join only where ASI cannot trigger
  kotlin  Kotlin: join only where the grammar allows a newline
  css     CSS family: whitespace is a descendant combinator inside selectors

`hazard` is a regex; when it matches the source, `hazard_allowed` replaces
`allowed` (e.g. shell heredocs, HTML <pre>).
"""
import fnmatch
import os
import re

ALL = frozenset({"blank", "indent", "join", "spaces"})
LINES = frozenset({"blank", "indent"})
BLANK = frozenset({"blank"})
NONE = frozenset()

LEVELS = {
    0: NONE,
    1: BLANK,
    2: LINES,
    3: frozenset({"blank", "indent", "join"}),
    4: ALL,
}


def L(lexer, exts, allowed, join=None, preproc=False, ts=None, fmt=(), names=(),
      hazard=None, hazard_allowed=NONE, indent_sensitive=False, protect=None, ts_verify=True):
    return dict(lexer=lexer, exts=tuple(exts), allowed=frozenset(allowed), join=join,
                ts_verify=ts if ts_verify else None,
                protect=re.compile(protect) if protect else None,
                preproc=preproc, ts=ts, fmt=tuple(fmt), names=tuple(names),
                hazard=re.compile(hazard, re.M) if hazard else None,
                hazard_allowed=frozenset(hazard_allowed),
                indent_sensitive=indent_sensitive)


LANGS = {
    # ---- brace / semicolon languages: everything is removable -------------
    "java":   L("java", [".java"], ALL, "free", ts="java",
                fmt=["clang-format@config", "google-java-format", "palantir-java-format",
                     "prettier-java", "clang-format"]),
    "c":      L("c", [".c", ".h"], ALL, "free", preproc=True, ts="c", fmt=["clang-format"]),
    "cpp":    L("cpp", [".cc", ".cpp", ".cxx", ".c++", ".hpp", ".hh", ".hxx", ".h++",
                        ".ipp", ".inl", ".tpp"], ALL, "free", preproc=True, ts="cpp",
                fmt=["clang-format"]),
    "objc":   L("objective-c", [".m"], ALL, "free", preproc=True, ts="objc",
                fmt=["clang-format"]),
    "objcpp": L("objective-c++", [".mm"], ALL, "free", preproc=True, fmt=["clang-format"]),
    "cuda":   L("cuda", [".cu", ".cuh"], ALL, "free", preproc=True, ts="cuda",
                fmt=["clang-format"]),
    "csharp": L("csharp", [".cs"], ALL, "free", preproc=True, ts="csharp",
                fmt=["csharpier", "dotnet-format", "clang-format"],
                # C# 11 raw strings are mis-lexed by Pygments: protect them as one token
                protect=r'\$*("{3,})[\s\S]*?\1(?!")'),
    "rust":   L("rust", [".rs"], ALL, "free", ts="rust", fmt=["rustfmt"]),
    # tree-sitter-go misreads one-line `if x {...}` as a composite literal; Go joins are
    # verified by re-deriving the lexer's semicolons instead
    "go":     L("go", [".go"], ALL, "go", ts="go", ts_verify=False, fmt=["gofmt"]),
    "php":    L("php", [".php"], ALL, "free", ts="php",
                fmt=["php-cs-fixer", "prettier-php", "phpcbf"],
                # inline HTML after ?> is output verbatim -> only trim blank lines
                hazard=r"\?>\s*\S", hazard_allowed=NONE),
    "dart":   L("dart", [".dart"], ALL, "free", ts="dart", fmt=["dart-format"]),
    "proto":  L("protobuf", [".proto"], ALL, "free", fmt=["clang-format", "buf-format"]),
    "glsl":   L("glsl", [".glsl", ".vert", ".frag", ".geom", ".comp"], ALL, "free",
                preproc=True, ts="glsl", fmt=["clang-format"]),
    "hlsl":   L("hlsl", [".hlsl", ".fx"], ALL, "free", preproc=True, ts="hlsl",
                fmt=["clang-format"]),
    "solidity": L("solidity", [".sol"], ALL, "free", ts="solidity",
                  fmt=["forge-fmt", "prettier-solidity"]),
    "json":   L("json", [".json", ".jsonc"], ALL, "free", ts="json",
                fmt=["biome@config", "prettier", "python-json"]),
    # ---- newline-sensitive, joined only where provably safe ---------------
    "javascript": L("javascript", [".js", ".mjs", ".cjs"], ALL, "js", ts="javascript",
                    fmt=["biome@config", "deno@config", "prettier"]),
    "jsx":    L("jsx", [".jsx"], ALL, "js", ts="javascript",
                fmt=["biome@config", "deno@config", "prettier"]),
    "typescript": L("typescript", [".ts", ".mts", ".cts"], ALL, "js", ts="typescript",
                    fmt=["biome@config", "deno@config", "prettier"]),
    "tsx":    L("tsx", [".tsx"], ALL, "js", ts="tsx",
                fmt=["biome@config", "deno@config", "prettier"]),
    "kotlin": L("kotlin", [".kt", ".kts"], ALL, "kotlin", ts="kotlin", ts_verify=False,
                fmt=["ktlint@config", "ktfmt", "ktlint"]),
    "css":    L("css", [".css"], ALL, "css", ts="css", fmt=["biome@config", "prettier"]),
    # Pygments doesn't always lex SCSS/Less `//` comments as comments (inside rules it
    # misses them), so joining lines could pull code into one; keep lines where they occur
    "scss":   L("scss", [".scss"], ALL, "css", ts="scss", fmt=["prettier"],
                hazard=r"(?<![:/])//", hazard_allowed=LINES),
    "less":   L("less", [".less"], ALL, "css", fmt=["prettier"],
                hazard=r"(?<![:/])//", hazard_allowed=LINES),
    # ---- newline-sensitive: keep line structure, drop indentation ---------
    "swift":  L("swift", [".swift"], LINES, ts="swift", fmt=["swift-format", "swiftformat"]),
    "groovy": L("groovy", [".groovy", ".gradle"], LINES, ts="groovy", fmt=["npm-groovy-lint"]),
    "sql":    L("sql", [".sql"], LINES, ts="sql", fmt=["sqlfluff", "sql-formatter"]),
    "lua":    L("lua", [".lua"], LINES, ts="lua", fmt=["stylua"]),
    "ruby":   L("ruby", [".rb", ".rake", ".gemspec"], LINES, ts="ruby",
                fmt=["rubocop", "rufo"]),
    "r":      L("r", [".r", ".R"], LINES, fmt=["styler"]),
    "bash":   L("bash", [".sh", ".bash", ".zsh", ".ksh"], LINES, ts="bash", fmt=["shfmt"],
                hazard=r"<<", hazard_allowed=NONE),
    "perl":   L("perl", [".pl", ".pm"], BLANK, fmt=["perltidy"],
                hazard=r"<<", hazard_allowed=NONE),
    "powershell": L("powershell", [".ps1", ".psm1"], BLANK, fmt=[],
                    hazard=r"@['\"]\s*$", hazard_allowed=NONE),
    "toml":   L("toml", [".toml"], LINES, ts="toml", fmt=["taplo"]),
    "xml":    L("xml", [".xml", ".xsd", ".xsl", ".xslt", ".svg", ".plist", ".csproj",
                        ".fsproj", ".vbproj"], LINES, ts="xml", fmt=["xmllint"],
                hazard=r"xml:space\s*=\s*['\"]preserve", hazard_allowed=NONE),
    "html":   L("html", [".html", ".htm", ".vue", ".svelte"], LINES, ts="html",
                fmt=["prettier"], hazard=r"<(pre|textarea)\b", hazard_allowed=NONE),
    "scala":  L("scala", [".scala", ".sc"], BLANK, ts="scala", fmt=["scalafmt"]),
    "zig":    L("zig", [".zig"], LINES, ts="zig", fmt=["zig-fmt"]),
    "clojure": L("clojure", [".clj", ".cljs", ".cljc", ".edn"], LINES, fmt=["cljfmt"]),
    "elixir": L("elixir", [".ex", ".exs"], LINES, ts="elixir", fmt=["mix-format"],
                hazard=r'"""|\'\'\'', hazard_allowed=BLANK),
    "verilog": L("verilog", [".v", ".sv", ".svh"], LINES, ts="verilog", fmt=["verible"]),
    "terraform": L("terraform", [".tf", ".tfvars", ".hcl"], LINES, fmt=["terraform-fmt"],
                   hazard=r"<<", hazard_allowed=NONE),
    "dockerfile": L("docker", [], LINES, names=["Dockerfile", "Containerfile"], fmt=[],
                    hazard=r"<<", hazard_allowed=NONE),
    # ---- indentation-sensitive: layout IS syntax ---------------------------
    "python": L("python", [".py", ".pyi", ".pyw"], BLANK, ts="python",
                fmt=["ruff", "black"], indent_sensitive=True),
    "haskell": L("haskell", [".hs"], BLANK, ts="haskell", fmt=["ormolu", "fourmolu"],
                 indent_sensitive=True),
    "fsharp": L("fsharp", [".fs", ".fsi", ".fsx"], BLANK, fmt=["fantomas"],
                indent_sensitive=True),
    "nim":    L("nim", [".nim"], BLANK, fmt=["nimpretty"], indent_sensitive=True),
    "coffeescript": L("coffeescript", [".coffee"], BLANK, fmt=[], indent_sensitive=True),
    "elm":    L("elm", [".elm"], BLANK, fmt=["elm-format"], indent_sensitive=True),
    "yaml":   L("yaml", [".yml", ".yaml"], NONE, ts="yaml", fmt=["prettier"],
                indent_sensitive=True),
    "make":   L("make", [".mk"], NONE, names=["Makefile", "makefile", "GNUmakefile"],
                fmt=[], indent_sensitive=True),
    "markdown": L("markdown", [".md", ".markdown"], NONE, fmt=["prettier"],
                  indent_sensitive=True),
    "rst":    L("rst", [".rst"], NONE, fmt=[], indent_sensitive=True),
    "pug":    L("pug", [".pug", ".jade"], NONE, fmt=[], indent_sensitive=True),
    "sass":   L("sass", [".sass"], NONE, fmt=[], indent_sensitive=True),
    "stylus": L("text", [".styl"], NONE, fmt=[], indent_sensitive=True),
}

ALIASES = {
    "js": "javascript", "node": "javascript", "ts": "typescript", "c++": "cpp",
    "cxx": "cpp", "cs": "csharp", "c#": "csharp", "kt": "kotlin", "rs": "rust",
    "golang": "go", "py": "python", "yml": "yaml", "sh": "bash", "shell": "bash",
    "zsh": "bash", "objective-c": "objc", "rb": "ruby", "protobuf": "proto",
    "docker": "dockerfile", "makefile": "make", "md": "markdown", "hcl": "terraform",
    "tf": "terraform", "golang-mod": "go",
}

_EXT = {}
_NAME = {}
for _k, _v in LANGS.items():
    for _e in _v["exts"]:
        _EXT.setdefault(_e, _k)
        _EXT.setdefault(_e.lower(), _k)
    for _n in _v["names"]:
        _NAME[_n] = _k

SKIP_DIRS = {".git", ".hg", ".svn", "node_modules", "target", "build", "dist", "out",
             ".gradle", ".idea", ".vscode", "__pycache__", ".venv", "venv", "vendor",
             ".next", ".nuxt", "bin", "obj", ".mypy_cache", ".pytest_cache", "coverage"}


def resolve_lang(name):
    if not name:
        return None
    n = name.strip().lower()
    n = ALIASES.get(n, n)
    return n if n in LANGS else None


def detect_language(path, forced=None):
    """Return a language key for `path` (or `forced`), or None if unknown."""
    if forced:
        return resolve_lang(forced)
    base = os.path.basename(path)
    if base in _NAME:
        return _NAME[base]
    if base.startswith("Dockerfile") or base.endswith(".dockerfile"):
        return "dockerfile"
    _, ext = os.path.splitext(base)
    return _EXT.get(ext) or _EXT.get(ext.lower())


def allowed_flags(lang, code):
    spec = LANGS[lang]
    if spec["hazard"] is not None and spec["hazard"].search(code):
        return spec["hazard_allowed"]
    return spec["allowed"]


def iter_source_files(root):
    """Yield supported source files under `root`, skipping vendored/build dirs."""
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS and not d.startswith("."))
        for f in sorted(filenames):
            p = os.path.join(dirpath, f)
            if detect_language(p):
                yield p


# ---------------------------------------------------------------- editorconfig
def editorconfig_indent(path):
    """Very small .editorconfig reader: returns (indent_style, indent_size) or (None, None)."""
    path = os.path.abspath(path)
    d = os.path.dirname(path)
    chain = []
    while True:
        ec = os.path.join(d, ".editorconfig")
        if os.path.isfile(ec):
            chain.append((d, ec))
            try:
                with open(ec, encoding="utf-8", errors="replace") as fh:
                    if re.search(r"^\s*root\s*=\s*true", fh.read(), re.M | re.I):
                        break
            except OSError:
                pass
        nd = os.path.dirname(d)
        if nd == d:
            break
        d = nd
    style = size = None
    for d, ec in reversed(chain):  # outermost first, innermost wins
        rel = os.path.relpath(path, d).replace(os.sep, "/")
        section_matches = False
        try:
            lines = open(ec, encoding="utf-8", errors="replace").read().splitlines()
        except OSError:
            continue
        for line in lines:
            s = line.strip()
            if not s or s[0] in "#;":
                continue
            if s.startswith("[") and s.endswith("]"):
                pat = s[1:-1]
                pats = _expand_braces(pat)
                section_matches = any(
                    fnmatch.fnmatch(rel, p if "/" in p else "*" + p) or
                    fnmatch.fnmatch(os.path.basename(rel), p) for p in pats)
                continue
            if section_matches and "=" in s:
                k, v = [x.strip().lower() for x in s.split("=", 1)]
                if k == "indent_style":
                    style = v
                elif k == "indent_size" and v.isdigit():
                    size = int(v)
    return style, size


def _expand_braces(pat):
    m = re.search(r"\{([^{}]*)\}", pat)
    if not m:
        return [pat]
    out = []
    for alt in m.group(1).split(","):
        out.extend(_expand_braces(pat[:m.start()] + alt + pat[m.end():]))
    return out


# ---------------------------------------------------------------- content sniffing
OBJC_HINT = re.compile(r"^\s*(@interface|@protocol|@class|@implementation|#import\s)", re.M)
CPP_HINT = re.compile(r"^\s*(namespace\s+\w|template\s*<|class\s+\w+[^;]*\{|using\s+namespace)"
                      r"|\bstd::", re.M)
JSX_HINT = re.compile(r"(?:^|[=(,?:&|]|return|=>)\s*<[A-Za-z][\w.]*[\s/>]|</[A-Za-z][\w.]*>", re.M)


def refine_language(lang, code):
    """Extensions lie: `.h` may be C++ or Objective-C, `.js` may contain JSX."""
    if lang == "c" and OBJC_HINT.search(code):
        return "objc"
    if lang == "c" and CPP_HINT.search(code):
        return "cpp"
    if lang == "javascript" and JSX_HINT.search(code):
        return "jsx"
    return lang
