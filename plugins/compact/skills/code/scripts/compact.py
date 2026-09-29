#!/usr/bin/env python3
"""compact.py - lexer-aware removal of code formatting to save LLM tokens.

Implements the "unformatted code" representation from arXiv:2508.13666
("The Hidden Cost of Readability"): indentation, newlines and optional
spaces are removed; strings, comments and preprocessor lines are kept intact,
and every result is re-lexed and compared with the original so that only
whitespace can ever change (automatic fallback to a safer level otherwise).

  compact.py Foo.java                    print compact code (level 3)
  cat Foo.java | compact.py -l java -    read stdin
  compact.py -L 4 src/                   whole tree, also drop optional spaces
  compact.py --outline 1 -n Big.java     one member per line + original line numbers
  compact.py --list-languages            show per-language safe levels

Levels: 0 as-is | 1 blank lines/trailing ws | 2 +indentation | 3 +newlines | 4 +optional spaces
Each language caps the level (Python/YAML/Makefile keep their layout).
Exit codes: 0 ok, 1 usage/IO error, 2 missing dependency.
"""
import argparse
import bisect
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from langs import (LANGS, LEVELS, allowed_flags, detect_language,  # noqa: E402
                   iter_source_files, refine_language, resolve_lang)

try:
    from pygments.lexers import get_lexer_by_name
    from pygments.token import Comment, Keyword, Literal, Name, Number, Operator, Punctuation
    from pygments.token import String, Token
except ImportError:  # pragma: no cover
    sys.stderr.write("compact.py: needs Pygments -> pip install pygments\n")
    sys.exit(2)

FLAG_ORDER = ["spaces", "join", "indent", "blank"]  # dropped in this order on fallback


class LexFail(Exception):
    pass


ALLOW_LEX_ERRORS = False  # only when an AST check can catch a confused lexer
JS_SEMI = True  # JS/TS: write ASI newlines as ';' only when an AST check will verify it
BENIGN_ERRORS = set("{}()[];, \t\n")


# ----------------------------------------------------------------- tokens
class Sig:
    """A significant (non-whitespace) token."""
    __slots__ = ("kind", "tt", "v", "start")

    def __init__(self, kind, tt, v, start):
        self.kind, self.tt, self.v, self.start = kind, tt, v, start

    def __repr__(self):
        return "Sig(%s,%r)" % (self.kind, self.v)


def _doc_comment(v):
    """String.Doc tokens that are really comments (///, //!, /** */, #) - not Python docstrings."""
    s = v.lstrip()
    return s.startswith("//") or s.startswith("/*") or s.startswith("#")


def is_stringish(tt, v):
    if tt is Token.Other:
        return True
    if tt in String.Doc:
        return not _doc_comment(v)
    return tt in String


def is_comment(tt, v):
    if tt in Comment.Preproc or tt in Comment.PreprocFile or tt in Comment.Hashbang:
        return False
    if tt in Comment:
        return True
    return tt in String.Doc and _doc_comment(v)


def make_lexer(spec, code):
    opts = dict(stripnl=False, stripall=False, ensurenl=False)
    if spec["lexer"] == "php" and "<?" not in code:
        opts["startinline"] = True
    return get_lexer_by_name(spec["lexer"], **opts)


def raw_tokens(code, lexer, protect):
    """(start, ttype, value) covering `code` exactly; protected regions become one String."""
    toks = []
    pos = 0
    for tt, v in lexer.get_tokens(code):
        if v:
            toks.append((pos, tt, v))
            pos += len(v)
    if pos != len(code) or "".join(t[2] for t in toks) != code:
        raise LexFail("lexer did not round-trip the source")
    if protect is None:
        return toks
    regions = [(m.start(), m.end()) for m in protect.finditer(code)]
    if not regions:
        return toks
    out, ri = [], 0
    for start, tt, v in toks:
        end = start + len(v)
        while ri < len(regions) and regions[ri][1] <= start:
            ri += 1
        if ri < len(regions) and regions[ri][0] < end and start < regions[ri][1]:
            rs, re_ = regions[ri]
            if start < rs:
                out.append((start, tt, code[start:rs]))
            if not out or out[-1][0] != rs:
                out.append((rs, String, code[rs:re_]))
            if end > re_:
                out.append((re_, tt, code[re_:end]))
                ri += 1
            continue
        out.append((start, tt, v))
    return out


def tokenize(code, spec, lexer):
    """Split into significant tokens and the whitespace gaps between them.

    Returns (sigs, gaps) with len(gaps) == len(sigs) + 1; gaps[i] precedes sigs[i].
    """
    toks = raw_tokens(code, lexer, spec.get("protect"))
    sigs, gaps, gap = [], [], []
    skip_to = -1
    line_start = True  # are we at the first significant token of a source line?
    i = 0
    while i < len(toks):
        start, tt, v = toks[i]
        i += 1
        if start + len(v) <= skip_to:
            continue
        if start < skip_to:  # token straddles the end of a verbatim preprocessor line
            v = v[skip_to - start:]
            start = skip_to
        if tt in Token.Error and not ALLOW_LEX_ERRORS and not set(v) <= BENIGN_ERRORS:
            # a confused lexer may have mis-read strings; a lone bracket (Pygments' Kotlin
            # lexer on `companion object {`) is harmless
            raise LexFail("lexer error token %r at offset %d" % (v[:20], start))
        if is_stringish(tt, v):
            gaps.append("".join(gap)); gap = []
            sigs.append(Sig("str", tt, v, start))
            line_start = False
            continue
        core = v.strip()
        if not core:
            gap.append(v)
            if "\n" in v:
                line_start = True
            continue
        lead = len(v) - len(v.lstrip())
        if lead:
            gap.append(v[:lead])
            if "\n" in v[:lead]:
                line_start = True
        cstart = start + lead
        if (spec["preproc"] and line_start and core.startswith("#")
                and (tt in Comment.Preproc or tt in Comment.PreprocFile)):
            end = _directive_end(code, cstart)
            gaps.append("".join(gap)); gap = []
            sigs.append(Sig("pre", tt, code[cstart:end].rstrip(), cstart))
            skip_to = end
            line_start = False
            if start + len(v) > end:  # e.g. Pygments' "#else\n" token inside #if 0 blocks
                toks.insert(i, (end, tt, code[end:start + len(v)]))
            continue
        kind = "com" if is_comment(tt, core) else "tok"
        if kind == "com" and not core.startswith("/*"):
            kind = "lc"  # line comment: the newline after it is mandatory
        gaps.append("".join(gap)); gap = []
        sigs.append(Sig(kind, tt, core, cstart))
        line_start = False
        trail = v[lead + len(core):]
        if trail:
            gap.append(trail)
            if "\n" in trail:
                line_start = True
    gaps.append("".join(gap))
    return sigs, gaps


def _directive_end(code, start):
    """End offset (exclusive, before the newline) of a preprocessor directive."""
    i = start
    while True:
        j = code.find("\n", i)
        if j == -1:
            return len(code)
        if code[j - 1] == "\\":
            i = j + 1
            continue
        # a /* comment opened on the directive line may span lines; keep it whole
        seg = code[start:j]
        if seg.count("/*") > seg.count("*/"):
            k = code.find("*/", j)
            if k != -1:
                i = k + 2
                continue
        return j


# ----------------------------------------------------------------- spacing rules
OPCH = set("+-*/%=<>!&|^~?:.@#\\")
# two operator characters that read as one operator when nothing separates them; a
# lexer may still split them (Pygments lexes `--` as two `-`), so the check doesn't
# rely on token bounds
FUSING = {"++", "--", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "::", "+=", "-=",
          "*=", "/=", "%=", "&=", "|=", "^=", "//", "/*", "*/", "**", "..", "?.", "??", "?:", "!!",
          "<-", ":="}
QUOTES = set("\"'`")


def word(c):
    return c.isalnum() or c in "_$" or ord(c) > 127


def need_space(p, n):
    a, b = p.v[-1], n.v[0]
    if word(a) and word(b):
        return True
    if a in OPCH and b in OPCH:
        return True
    if b == "." and (p.tt in Number or a.isdigit()):
        return True
    if a == "." and b.isdigit():
        return True
    if (word(a) and b in QUOTES) or (a in QUOTES and word(b)):
        return True
    if word(a) and b == "#":
        return True  # Rust 2021 reserves `ident#` (and C/C# `x#` reads badly anyway)
    if n.kind in ("com", "lc"):
        return True
    return False


def css_sep(p, n):
    if p.v[-1] in "{};,:" or n.v[0] in "{};,":
        return ""
    return " "


def _opchar_tok(t):
    return t.kind == "tok" and (t.tt in Operator or t.tt in Punctuation) and \
        all(c in OPCH for c in t.v)


def punct(t):
    return t.kind == "tok" and (t.tt in Operator or t.tt in Punctuation)


# JavaScript / TypeScript: joining a newline is safe unless ASI fires there.
JS_END = {"{", "(", "[", ",", ";", ".", "?.", "...", "=>", "=", "+=", "-=", "*=", "/=",
          "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=", "??=", "&&",
          "||", "??", "?", ":", "+", "-", "*", "/", "%", "**", "&", "|", "^", "!", "~",
          "<", "==", "===", "!=", "!==", "<=", ">=", "<<"}  # not ">>": TS `Array<Array<T>>`
JS_END_WORDS = {"instanceof", "in", "typeof", "delete", "new", "else", "extends",
                "implements", "case", "do", "try", "finally"}
JS_START = {")", "]", "}", ",", ";", ".", "?.", ":", "?", "&&", "||", "??", "==", "===",
            "!=", "!==", "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=",
            "&=", "|=", "^=", "&&=", "||=", "??=", "|", "&", "*", "/", "%", "<", ">", ">=",
            "<=", "<<", ">>", ">>>", "**", "^"}
JS_START_WORDS = {"instanceof"}
AFTER_CLOSE_WORDS = {"else", "catch", "finally", "while"}  # safe to join after "}"
JS_RESTRICTED = {"return", "throw", "break", "continue", "yield", "async", "await", "get",
                 "set", "static", "let", "abstract", "declare", "type", "namespace",
                 "module", "readonly", "override", "accessor"}

OBJ_BEFORE = {"(", ",", "=", ":", "[", "?", "||", "&&", "??", "return", "...", "<",
              "default"}  # a `{` after these starts an object literal
HEADER_WORDS = {"if", "for", "while", "when", "catch", "switch", "with", "foreach"}
JS_NO_STMT_START = {"in", "instanceof", "of", "as", "satisfies", "from", "extends", "implements",
                    "else", "catch", "finally", "keyof", "is"}
JS_CONTEXTUAL = {"static", "get", "set", "async", "abstract", "declare", "readonly", "public",
                 "private", "protected", "override", "accessor", "export", "default", "let",
                 "type", "namespace", "module", "interface", "enum", "as", "satisfies", "of",
                 "in", "from", "new", "typeof", "void", "delete", "await", "case", "var", "const",
                 "function", "class", "extends", "implements", "import", "else", "do", "try"}
KT_MODIFIERS = {"private", "public", "internal", "protected", "override", "open", "final",
                "abstract", "lateinit", "const", "inline", "suspend", "operator", "infix",
                "tailrec", "external", "actual", "expect", "inner", "data", "sealed", "enum",
                "annotation", "companion", "value", "vararg", "noinline", "crossinline",
                "reified", "fun", "val", "var", "class", "object", "interface", "typealias",
                "import", "package", "in", "out", "is", "as", "by", "where", "constructor",
                "get", "set", "init", "else", "if", "when", "try", "do", "for", "while"}
KT_NO_STMT_START = {"get", "set", "by", "where", "constructor", "as", "is", "in", "else",
                    "catch", "finally", "out"}
KT_END = {"{", "(", "[", ",", ";", "=", "+=", "-=", "*=", "/=", "%=", "->", "&&", "||",
          "?:", "+", "-", "*", "/", "%", "..", "..<", "<", "==", "!=", "===", "!==", "<=",
          ">=", ":", "?.", "::"}
KT_END_WORDS = {"else", "in", "is", "as", "try", "finally", "do"}
KT_START = {")", "]", "}", ",", ".", "?.", "?:", "&&", "||", "->", ":", "=", ";"}
KT_START_WORDS = {"catch", "finally"}  # not "else": in `when`, a newline before else matters

GO_KEYWORDS = {"break", "case", "chan", "const", "continue", "default", "defer", "else",
               "fallthrough", "for", "func", "go", "goto", "if", "import", "interface",
               "map", "package", "range", "return", "select", "struct", "switch", "type",
               "var"}


def go_needs_semi(p):
    """Would the Go lexer insert a semicolon after token p at a newline?"""
    if p.kind == "str":
        return True
    if p.kind != "tok":
        return False
    v = p.v
    if v in ("break", "continue", "fallthrough", "return", "++", "--", ")", "]", "}"):
        return True
    if v in GO_KEYWORDS:
        return False
    return (p.tt in Name or p.tt in Literal or p.tt in Keyword.Type
            or p.tt in Keyword.Constant) and (word(v[-1]) or p.tt in Literal)


# a JSX element can only start where an expression can (not `createContext<T>` generics)
JSX_BEFORE = {"(", ",", "=", "return", "=>", "?", ":", "&&", "||", "??", "{", "[", ";", "}",
              "default", "yield", "await"}


def jsx_text_positions(sigs, match):
    """Indices of tokens that are JSX text children (where a ';' would become visible text)."""
    out, tags, stack = set(), set(), []  # items: "tag", "close", "kids", ("expr", closing idx)
    for i, t in enumerate(sigs):
        top = stack[-1] if stack else None
        if isinstance(top, tuple) and i == top[1]:
            stack.pop()  # end of a {...} expression container
            continue
        v = t.v if t.kind == "tok" else None
        nxt = sigs[i + 1] if i + 1 < len(sigs) else None
        prev = sigs[i - 1] if i else None
        opens_tag = v == "<" and nxt is not None and (nxt.tt in Name.Tag or nxt.v == ">") and (
            top == "kids" or isinstance(top, tuple) or prev is None or prev.v in JSX_BEFORE)
        if opens_tag and top not in ("tag", "close"):
            stack.append("tag")
        elif v == "</" and top not in ("tag", "close"):
            stack.append("close")
        elif top == "tag" and (v == "/>" or (v == "/" and nxt is not None and nxt.v == ">")):
            stack.pop()
            if v == "/":
                stack.append("skip>")  # Pygments sometimes splits "/>" into "/" ">"
        elif top == "skip>":
            stack.pop()
        elif top == "tag" and v == ">":
            stack[-1] = "kids"
        elif top == "close" and v == ">":
            stack.pop()
            if stack and stack[-1] == "kids":
                stack.pop()
        elif top == "kids":
            if v == "{" and i in match:
                stack.append(("expr", match[i]))
            else:
                out.add(i)
        elif top == "tag":
            if v == "{" and i in match:
                stack.append(("expr", match[i]))  # attribute value expression
            else:
                tags.add(i)
    return out, tags


def semicolon_style(sigs, gaps):
    """True when (JS/TS) statements are terminated with ';' (prettier default)."""
    semi = bare = 0
    for k in range(1, len(sigs)):
        if "\n" not in gaps[k]:
            continue
        p, n = sigs[k - 1], sigs[k]
        if p.kind in ("lc", "com", "pre"):
            continue
        if p.v == ";":
            semi += 1
        elif (p.kind == "str" or p.tt in Name or p.tt in Literal or p.v in (")", "]")) \
                and not (punct(n) and n.v in JS_START) and not p.v.startswith("@"):
            bare += 1
    return semi >= 3 and bare <= semi * 0.15


class Ctx:
    def __init__(self, spec, sigs, gaps, flags):
        self.spec, self.flags = spec, flags
        self.join = spec["join"]
        self.sigs, self.gaps = sigs, gaps
        self.pseudo_comment = set()
        self.go_imports = set()  # token indices inside Go `import ( ... )` groups
        # Pygments splits some operators into characters (Kotlin: "+", "+"); re-fuse them
        self.op_end, self.op_start = {}, {}
        i = 0
        while i < len(sigs):
            if _opchar_tok(sigs[i]):
                j = i
                while j + 1 < len(sigs) and _opchar_tok(sigs[j + 1]) and not gaps[j + 1]:
                    j += 1
                text = "".join(t.v for t in sigs[i:j + 1])
                self.op_end[j], self.op_start[i] = (text, i), (text, j)
                i = j + 1
            else:
                i += 1
        self.nojoin = set()
        self.match = {}
        stack = []
        for i, t in enumerate(sigs):
            if punct(t) and t.v in ("(", "[", "{"):
                stack.append(i)
            elif punct(t) and t.v in (")", "]", "}") and stack:
                o = stack.pop()
                self.match[i] = o
                self.match[o] = i
        self.js_semi = self.join == "js" and semicolon_style(sigs, gaps)
        if self.join == "go":
            for i, t in enumerate(sigs[:-1]):
                if t.v == "import" and sigs[i + 1].v == "(" and i + 1 in self.match:
                    self.go_imports.update(range(i + 2, self.match[i + 1]))
        self.jsx_text, self.jsx_tag = jsx_text_positions(sigs, self.match) \
            if spec["lexer"] in ("jsx", "tsx") else (set(), set())
        # indices of block comments that sit on their own line in the source
        self.own_line = {k for k, t in enumerate(sigs)
                         if t.kind == "com" and (k == 0 or "\n" in gaps[k])
                         and (k + 1 >= len(gaps) or "\n" in gaps[k + 1])}
        self.lead_comment = {k for k, t in enumerate(sigs) if t.kind == "com" and "\n" in gaps[k]}
        for k, t in enumerate(sigs):
            if t.kind == "tok" and t.v.startswith("<?") and k + 1 < len(sigs):
                self.nojoin.add(k + 1)
        # `//` the lexer did not recognise as a comment (Pygments inside JSX tags): the rest
        # of that source line is a comment, so its newline must stay
        if self.join in ("js", "kotlin", "free", "go"):
            for i, t in enumerate(sigs):
                if t.kind == "tok" and (t.v.startswith("//") or (
                        t.v == "/" and i + 1 < len(sigs) and sigs[i + 1].v.startswith("/")
                        and not gaps[i + 1])):
                    j = i + 1
                    while j < len(sigs) and "\n" not in gaps[j]:
                        j += 1
                    self.nojoin.add(j)
                    self.pseudo_comment.update(range(i, j))
        # after a heredoc/nowdoc closing delimiter the newline is mandatory (PHP < 7.3)
        for k, t in enumerate(sigs[:-1]):
            if t.kind == "str" and t.tt in String.Delimiter and sigs[k + 1].kind != "str":
                self.nojoin.add(k + 1)
                if sigs[k + 1].v == ";":
                    self.nojoin.add(k + 2)

    def sep(self, p, n):
        if self.join == "css":
            return css_sep(p, n)
        if self.join == "js" and p.v.endswith(">") and n.v.startswith("<"):
            return ""  # JSX: a space between two tags would become a text node
        return " " if need_space(p, n) else ""

    def import_group_break(self, p, n, k):
        """A blank line between two groups of imports: formatters sort *within* groups, so
        dropping it would reorder the imports after a round trip."""
        if k is None:
            return False
        if n.kind == "pre" and p.kind == "pre":
            return n.v.lstrip("# \t").startswith(("include", "import")) and \
                p.v.lstrip("# \t").startswith(("include", "import"))
        nv = n.v
        if nv in ("pub", "export") and k + 1 < len(self.sigs):
            nv = self.sigs[k + 1].v
        if nv in ("use", "import", "from", "using") and p.kind == "tok" and p.v in (";", "}") \
                or (self.join == "kotlin" and nv == "import"):
            return True
        if self.spec["lexer"] == "rust":
            j = k  # skip #[attributes] and visibility to the item keyword
            while j < len(self.sigs):
                t = self.sigs[j]
                if t.v.startswith("#") and "[" in t.v:
                    depth = 0
                    while j < len(self.sigs):
                        if self.sigs[j].kind != "str":
                            depth += self.sigs[j].v.count("[") - self.sigs[j].v.count("]")
                        j += 1
                        if depth <= 0:
                            break
                    continue
                if t.v in ("pub", "(", ")", "crate", "super", "in"):
                    j += 1
                    continue
                break
            return j < len(self.sigs) and self.sigs[j].v in ("use", "extern", "mod")
        return k in self.go_imports

    def pv(self, k):
        """Text of the (fused) token ending at index k."""
        return self.op_end[k][0] if k in self.op_end else self.sigs[k].v

    def nv(self, k):
        """Text of the (fused) token starting at index k."""
        return self.op_start[k][0] if k in self.op_start else self.sigs[k].v

    def annotation_end(self, i):
        """Is token i the end of `@Name`, `@get:Name`, `@a.b.Name` (no arguments)?"""
        j = i
        while j >= 0 and i - j <= 8:
            t = self.sigs[j]
            if t.v.startswith("@") or t.tt in Name.Decorator:
                return True
            if not (t.v in (".", ":") or (t.kind == "tok" and t.tt in Name)) or \
                    (j < i and self.gaps[j + 1]):
                return False
            j -= 1
        return False

    def can_join(self, p, n, k=None):
        if p.kind in ("lc", "pre") or n.kind in ("pre", "lc") or k in self.nojoin:
            return False  # an own-line // comment stays on its own line (it annotates the next one)
        if k is not None and (k in self.own_line or k - 1 in self.own_line
                              or (n.kind == "com" and k in self.lead_comment)):
            return False  # own-line / line-leading /* */ comments keep their line (placement)
        if self.join == "js" and p.v == "{" and k is not None and k >= 2 and \
                (self.sigs[k - 2].v in OBJ_BEFORE or
                 (k + 1 < len(self.sigs) and self.sigs[k + 1].v == ":" and n.kind in ("tok", "str"))):
            return False  # expanded object literal: prettier keeps it expanded only if we do
        j = self.join
        if j in ("free", "go", "css"):
            return True
        if k is None:
            return False
        pv, nv = self.pv(k - 1), self.nv(k)
        if j == "js":
            if pv in JS_RESTRICTED or nv in ("++", "--") or pv in ("++", "--"):
                return False
            if (punct(p) and pv in JS_END) or (p.kind == "tok" and pv in JS_END_WORDS):
                return True
            if (punct(n) and nv in JS_START) or (n.kind == "tok" and nv in JS_START_WORDS):
                return True
            if punct(p) and pv == "}" and (self.js_semi or nv in AFTER_CLOSE_WORDS) \
                    and self.js_block_close(k - 1):
                return True  # `}` of a block/declaration: nothing for ASI to do here
            return False
        if j == "kotlin":
            if pv in ("++", "--", "!!") or nv in ("++", "--"):
                return False
            if self.kt_infix(k - 1):
                return True  # the grammar allows NL* after an infix function name
            if (punct(p) and pv in KT_END) or (p.kind == "tok" and pv in KT_END_WORDS):
                return True
            if (punct(n) and nv in KT_START) or (n.kind == "tok" and nv in KT_START_WORDS):
                return True
            if punct(p) and pv == "}" and nv in AFTER_CLOSE_WORDS and nv != "while":
                return True
            return False
        return False

    def semi_join(self, p, n, k):
        """Can this newline be written as ';'? Only where the language would end the
        statement there anyway (JS ASI / Kotlin newline-as-separator) - never inside a
        control header, before a continuation, after an annotation, or in JSX text."""
        if k is None or self.join not in ("js", "kotlin") or (self.join == "js" and not JS_SEMI) \
                or k in self.jsx_text \
                or k - 1 in self.jsx_text or k in self.jsx_tag or k - 1 in self.jsx_tag \
                or not self.value_end(p, k - 1):
            return False
        if n.kind == "str":
            return n.v[:1] in ("'", '"') and self.join == "js" or \
                (self.join == "kotlin" and n.v[:1] == '"')
        if n.kind != "tok":
            return False
        v = self.nv(k)
        if self.join == "js":
            if v in JS_NO_STMT_START or n.tt in String:
                return False
            if v in ("!", "~", "++", "--") or n.tt in Name.Decorator or v.startswith("#"):
                return True
            if n.tt in Number:
                return not v.startswith(".")
            return (n.tt in Name or n.tt in Keyword) and word(v[0])
        # kotlin
        if v in KT_NO_STMT_START:
            return False
        j = k
        while j < len(self.sigs) and j < k + 12:
            t = self.sigs[j]
            if t.v.startswith("@") or t.tt in Name.Decorator or \
                    (t.v in (".", ":") and self.gaps[j] == "") or \
                    (t.tt in Name and j > k and self.sigs[j - 1].v in (".", ":", "@")):
                j += 1
                if j < len(self.sigs) and self.sigs[j].v == "(" and j in self.match:
                    j = self.match[j] + 1  # skip annotation arguments
                continue
            if t.v in KT_MODIFIERS and t.v not in KT_NO_STMT_START:
                j += 1
                continue
            break
        if j < len(self.sigs) and self.sigs[j].v in KT_NO_STMT_START:
            return False  # `private set`, `@Inject constructor(...)` belong to what is above
        if v in ("!", "++", "--") or v.startswith("@"):
            return True
        return (n.tt in Name or n.tt in Keyword or n.tt in Number) and word(v[0])

    def js_block_close(self, i):
        """Is the `}` at index i the end of a block or declaration (vs. an object literal,
        arrow body or function expression, which end an expression that ASI terminates)?"""
        o = self.match.get(i)
        if o is None or o == 0:
            return o == 0
        prev = self.sigs[o - 1]
        if prev.v in OBJ_BEFORE or prev.v == "=>":
            return False
        if prev.v == ")":
            po = self.match.get(o - 1)
            if po is None or po == 0:
                return True
            j = po - 1
            if self.sigs[j].v in HEADER_WORDS:
                return True
            if self.sigs[j].kind == "tok" and self.sigs[j].tt in Name and j > 0:
                j -= 1
            if self.sigs[j].v == "*" and j > 0:
                j -= 1
            if self.sigs[j].v == "function":
                q = self.sigs[j - 1] if j > 0 else None
                if q is not None and q.v == "async":
                    q = self.sigs[j - 2] if j > 1 else None
                return not (q is not None and (q.v in OBJ_BEFORE or q.v == "=>"))
            return True
        if prev.v == "class" or (o >= 2 and self.sigs[o - 2].v == "class"):
            q = self.sigs[o - 3] if prev.v != "class" and o >= 3 else (
                self.sigs[o - 2] if o >= 2 else None)
            return not (q is not None and q.v in OBJ_BEFORE)
        return True

    def value_end(self, p, i):
        """Does token p (at index i) end an expression/statement?"""
        if p.kind == "str":
            # not the `${` / `{` that opens a template interpolation
            return not (p.tt in String.Interpol and p.v.endswith("{"))
        if p.kind != "tok" or p.v.startswith("@") or p.tt in Name.Decorator:
            return False
        v = self.pv(i)
        if self.join == "kotlin" and v in ("return", "throw", "break", "continue"):
            return False
        if v == ")":
            o = self.match.get(i)
            if o is None or o == 0:
                return False
            before = self.sigs[o - 1]
            if before.v in HEADER_WORDS:
                return False
            return not self.annotation_end(o - 1)  # @Ann(args), @get:Ann(args), @dec(args)
        if v in ("++", "--"):
            # postfix only if glued to its operand on the same line (`i\n++\nj` is `i; ++j`)
            s0 = self.op_end[i][1] if i in self.op_end else i
            return s0 > 0 and "\n" not in self.gaps[s0] and \
                (self.sigs[s0 - 1].v in (")", "]") or self.sigs[s0 - 1].tt in Name)
        if v in ("]", "}") or (self.join == "kotlin" and v in ("!!", "?")):
            return True
        if self.join == "js" and v in ("return", "break", "continue", "throw", "yield",
                                       "debugger", "this", "super", "null", "true", "false"):
            return True  # restricted productions: ASI puts the ';' right here anyway
        if v == "void" and self.spec["lexer"] in ("typescript", "tsx"):
            return True  # `(): void` at the end of a TS member
        if v in JS_CONTEXTUAL if self.join == "js" else v in KT_MODIFIERS:
            return False
        if self.annotation_end(i):
            return False
        if self.join == "kotlin" and self.kt_infix(i):
            return False  # `a to\nb`, `x or\ny`: an infix call continues on the next line
        return (p.tt in Name or p.tt in Literal or p.tt in Keyword.Constant
                or p.tt in Keyword.Type) and word(v[-1])

    def kt_infix(self, i):
        """Kotlin: is identifier i used as an infix function (`value name`)?"""
        p = self.sigs[i]
        if i < 1 or not (p.kind == "tok" and p.tt in Name) or not self.gaps[i] \
                or "\n" in self.gaps[i]:
            return False
        q = self.sigs[i - 1]
        if q.kind == "str" or q.v in (")", "]", "}", "!!"):
            return True
        return q.kind == "tok" and (q.tt in Name or q.tt in Number or q.tt in Keyword.Constant
                                    or q.v in ("true", "false", "null", "this")) \
            and q.v not in KT_MODIFIERS and q.v not in ("return", "throw") \
            and not self.annotation_end(i - 1)

    def join_sep(self, p, n, last_code=None):
        if self.join == "go":
            q = last_code if p.kind == "com" and last_code is not None else p
            if go_needs_semi(q) and n.v not in (")", "}", ";"):
                return ";" if p.kind != "com" else " ;"
        return self.sep(p, n)


# ----------------------------------------------------------------- rendering
DIRECTIVE = re.compile(r"^(//|#|--)\s*(go:|\+build|nolint|noqa|type:|pylint:|eslint|"
                       r"@ts-|prettier-ignore|clang-format|NOLINT|region|endregion|"
                       r"pragma|@flow|swiftlint|ktlint|rubocop|fmt:)|^/\*[!@]|^#!")


def render(code, spec, lexer, flags, strip_comments=False, outline=None, wrap=0,
           numbers=False):
    sigs, gaps = tokenize(code, spec, lexer)
    if strip_comments:
        sigs, gaps = _strip_comments(sigs, gaps)
    ctx = Ctx(spec, sigs, gaps, flags)
    nl_offsets = [m.start() for m in re.finditer("\n", code)]

    def lineno(off):
        return bisect.bisect_right(nl_offsets, off - 1) + 1

    out = []
    line_len = 0
    src_line = [None]  # original line number of each output line (None = blank/unknown)
    depth = 0
    interp = 0  # inside ${...} of a template string: formatters leave it alone, so do we
    last_code = None  # last non-comment token (Go semicolons look through comments)
    for k, t in enumerate(sigs):
        gap = gaps[k]
        if k == 0:
            s = gap[gap.rfind("\n") + 1:] if "indent" not in flags else ""
            if not flags & {"blank", "indent"}:
                s = gap
        else:
            s = _gap(gap, sigs[k - 1], t, ctx, flags - {"spaces"} if interp else flags, depth,
                     outline, last_code, k)
            if wrap and line_len >= wrap and "\n" in gap and "\n" not in s:
                s = s.rstrip(" ") + "\n"  # restoring an original line break is always safe
        out.append(s)
        if "\n" in s:
            line_len = len(s) - s.rfind("\n") - 1
            src_line.extend([None] * s.count("\n"))
        else:
            line_len += len(s)
        first = lineno(t.start)
        if src_line[-1] is None:
            src_line[-1] = first
        out.append(t.v)
        if "\n" in t.v:
            line_len = len(t.v) - t.v.rfind("\n") - 1
            src_line.extend(first + j + 1 for j in range(t.v.count("\n")))  # verbatim lines
        else:
            line_len += len(t.v)
        if t.kind not in ("com", "lc"):
            last_code = t
        if t.kind == "str" and t.tt in String.Interpol:
            if t.v.endswith("{"):
                interp += 1
            elif t.v == "}" and interp:
                interp -= 1
        if punct(t):
            if t.v == "{":
                depth += 1
            elif t.v == "}":
                depth = max(0, depth - 1)
    if sigs:
        tail = gaps[-1]
        out.append("\n" if flags & {"blank", "indent"} or not tail else tail)
    text = "".join(out)
    if numbers:
        lines = text.split("\n")
        width = len(str(max([x for x in src_line if x] or [1])))
        numbered = []
        for idx, ln in enumerate(lines):
            num = src_line[idx] if idx < len(src_line) else None
            if ln and num:
                numbered.append("%*d|%s" % (width, num, ln))
            elif ln:
                numbered.append(" " * width + "|" + ln)
            else:
                numbered.append(ln)
        text = "\n".join(numbered)
    return text


def _gap(gap, p, n, ctx, flags, depth, outline, last_code=None, k=None):
    nl = gap.count("\n")
    if k is not None and not nl and (k in ctx.pseudo_comment and k - 1 in ctx.pseudo_comment):
        return gap  # inside a `//` comment the lexer missed: it is text, leave it
    jsx_ok = k is not None and p.kind not in ("lc", "pre") and n.kind not in ("lc", "pre") \
        and not (k in ctx.own_line or k - 1 in ctx.own_line) and k not in ctx.nojoin \
        and k not in ctx.pseudo_comment and k - 1 not in ctx.pseudo_comment
    if jsx_ok and nl and "join" in flags and (k in ctx.jsx_tag or k - 1 in ctx.jsx_tag) \
            and p.v not in ("<", "</") and n.v not in (">", "/>", "/"):
        return " "  # between JSX attributes a newline is just a separator
    if jsx_ok and nl and "join" in flags and ((k in ctx.jsx_text) != (k - 1 in ctx.jsx_text)) \
            and (k in ctx.jsx_text or k - 1 in ctx.jsx_text):
        return ""  # JSX drops whitespace-with-newline between text and a tag or {expr}
    if jsx_ok and gap and not nl and (k in ctx.jsx_text or k - 1 in ctx.jsx_text):
        return gap  # a space next to JSX text is text
    if jsx_ok and gap and k in ctx.jsx_text and k - 1 in ctx.jsx_text:
        # JSX text between two words: leave it alone (only the indentation of a new line
        # is insignificant). Pygments mis-detects JSX now and then, so be conservative.
        return gap if not nl else "\n" + ("" if "indent" in flags else gap[gap.rfind("\n") + 1:])
    if nl == 0:
        if not gap:
            return ""
        if "spaces" in flags:
            return ctx.sep(p, n)
        if "indent" in flags:
            return " "
        return gap
    keep = ("join" not in flags or not ctx.can_join(p, n, k)
            or (outline is not None and depth <= outline))
    # a blank line next to a comment separates comment groups (Go build constraints, doc
    # comments): keep it even when blank lines are otherwise dropped
    para = nl >= 2 and ("para" in flags or p.kind in ("com", "lc") or n.kind in ("com", "lc")
                        or ctx.import_group_break(p, n, k))
    if not keep and not para:
        return ctx.join_sep(p, n, last_code)
    if (not para and "join" in flags and not (outline is not None and depth <= outline)
            and "\n" in gap and ctx.semi_join(p, n, k)):
        return ";"  # the newline ended a statement: say so explicitly
    if not flags & {"blank", "indent"}:
        return gap
    head = gap[:gap.rfind("\n")]
    indent = "" if "indent" in flags else gap[gap.rfind("\n") + 1:]
    if para:
        return "\n\n" + indent  # keep one blank line: formatters treat it as intent
    if "blank" in flags:
        return "\n" + indent
    # keep blank lines, drop trailing whitespace on them
    return "\n" * head.count("\n") + "\n" + indent


def _strip_comments(sigs, gaps):
    ns, ng = [], [gaps[0]]
    for k, t in enumerate(sigs):
        if t.kind in ("com", "lc") and not DIRECTIVE.search(t.v):
            # a multi-line block comment acts as a line break (Go/JS semicolon rules)
            glue = "\n" if "\n" in t.v else " "
            ng[-1] = ng[-1] + glue + gaps[k + 1]
            continue
        ns.append(t)
        ng.append(gaps[k + 1])
    return ns, ng


# ----------------------------------------------------------------- verification
def _norm_pre(v):
    # formatters realign `\` continuations and indent nested directives ("#  define")
    v = re.sub(r"[ \t]*\\\n[ \t]*", "\\\n", v.rstrip())
    return re.sub(r"(?<=#)[ \t]+", "", re.sub(r"[ \t]+", " ", v))


def lex_view(code, spec, lang, strip_comments=False, comment_chars=True):
    """Whitespace-free view of the token stream used by the lexical safety check.

    stream   non-whitespace characters of all tokens, in order (Go: without ';')
    bounds   positions in `stream` where one token ends and the next begins
    gapped   the subset of bounds that had whitespace in between in this source
    comments comment texts in order; pres: preprocessor lines in order
    mstrings multi-line string literals in order (their inner whitespace is content)
    """
    sigs, gaps = tokenize(code, spec, make_lexer(spec, code))
    if lang == "go":
        sigs, gaps = _go_effective(sigs, gaps)
    stream, bounds, gapped, nlgap = [], set(), set(), set()
    comments, pres, mstrings = [], [], []
    for k, t in enumerate(sigs):
        if t.kind in ("com", "lc"):
            if strip_comments and not DIRECTIVE.search(t.v):
                continue
            comments.append(" ".join(t.v.split()))
            if not comment_chars:
                continue
        elif t.kind == "pre":
            pres.append(_norm_pre(t.v))
        elif t.kind == "str" and "\n" in t.v:
            mstrings.append(t.v)
        pieces = t.v.split()
        for j, piece in enumerate(pieces):
            bounds.add(len(stream))
            if j or gaps[k]:
                gapped.add(len(stream))
            if not j and "\n" in gaps[k]:
                nlgap.add(len(stream))
            stream.extend(piece)
    return "".join(stream), bounds, gapped, comments, pres, mstrings, nlgap


def _go_effective(sigs, gaps):
    """Go token stream with the semicolons the Go lexer inserts at newlines made explicit
    (and the optional ones before ')' / '}' removed), so that joins can be checked
    independently of the renderer's own semicolon logic."""
    out, ogaps = [], []
    last = None
    for k, t in enumerate(sigs):
        nl = "\n" in gaps[k] or (out and out[-1].kind in ("lc", "com") and "\n" in out[-1].v)
        if nl and last is not None and go_needs_semi(last) and last.v != ";":
            out.append(Sig("tok", Punctuation, ";", t.start))
            ogaps.append("")
            last = out[-1]
        out.append(t)
        ogaps.append(gaps[k])
        if t.kind not in ("com", "lc"):
            last = t
    if last is not None and go_needs_semi(last):
        out.append(Sig("tok", Punctuation, ";", len(out)))
        ogaps.append("")
    ogaps.append(gaps[-1] if gaps else "")
    # drop semicolons that are optional (before a closer) and duplicates
    res, rgaps = [], []
    code_toks = [i for i, t in enumerate(out) if t.kind not in ("com", "lc")]
    nxt = {code_toks[j]: out[code_toks[j + 1]] for j in range(len(code_toks) - 1)}
    for i, t in enumerate(out):
        if t.kind == "tok" and t.v == ";":
            n = nxt.get(i)
            if n is None or n.v in (")", "}") or (n.kind == "tok" and n.v == ";"):
                continue
        res.append(t)
        rgaps.append(ogaps[i])
    rgaps.append(ogaps[-1])
    return res, rgaps


def literal_view(code, spec, lang, strip_comments=False):
    """String literals of `code` in order, and the `lex_view` stream positions that
    come from comments, strings or preprocessor lines (not code)."""
    sigs, gaps = tokenize(code, spec, make_lexer(spec, code))
    if lang == "go":
        sigs, gaps = _go_effective(sigs, gaps)
    strings, literal, n = [], set(), 0
    for t in sigs:
        if t.kind in ("com", "lc") and strip_comments and not DIRECTIVE.search(t.v):
            continue
        size = len("".join(t.v.split()))
        if t.kind in ("com", "lc", "str", "pre"):
            literal.update(range(n, n + size))
        if t.kind == "str":
            strings.append(t.v)
        n += size
    return strings, literal


def strings_kept(orig_strings, new):
    """Every string literal of the original appears verbatim, in order, in `new` —
    independent of how a lexer splits the compact text."""
    pos = 0
    for v in orig_strings:
        i = new.find(v, pos)
        if i < 0:
            return v
        pos = i + len(v)
    return None


def lexical_diff(orig, new, spec, lang, strip_comments=False):
    """None if `new` differs from `orig` only in layout, else a short reason."""
    s0, b0, g0, c0, p0, m0, nl0 = lex_view(orig, spec, lang, strip_comments)
    s1, b1, g1, c1, p1, m1, _ = lex_view(new, spec, lang, strip_comments)
    if s0 != s1:
        if spec["join"] not in ("js", "kotlin"):
            i = next((i for i, (x, y) in enumerate(zip(s0, s1)) if x != y), min(len(s0), len(s1)))
            return "chars differ near %r" % s0[max(0, i - 15):i + 15]
        # JS/Kotlin: a ';' may stand where the original had a statement-ending newline
        i = j = 0
        mapped, mapped_gaps = set(), set()
        while i < len(s0) or j < len(s1):
            if j < len(s1) and j in b1:
                mapped.add(i)
            if j < len(s1) and j in g1:
                mapped_gaps.add(i)
            if i < len(s0) and j < len(s1) and s0[i] == s1[j]:
                i += 1
                j += 1
            elif j < len(s1) and s1[j] == ";" and i in nl0:
                j += 1
                mapped.add(i)
                mapped_gaps.add(i)
            else:
                return "chars differ near %r" % s0[max(0, i - 15):i + 15]
        b1, g1 = mapped, mapped_gaps
    strings0, literal0 = literal_view(orig, spec, lang, strip_comments)
    for b in sorted(g0 - g1):
        if b - 1 not in literal0 and b not in literal0 and s0[b - 1] + s0[b] in FUSING:
            return "operators fused near %r" % s0[max(0, b - 15):b + 15]
    for b in sorted(g0 - b1):  # a separation that existed is gone: did two tokens fuse?
        a, c = s0[b - 1], s0[b]
        if (word(a) and word(c)) or (a in OPCH and c in OPCH and a + c != "><"):
            return "tokens fused near %r" % s0[max(0, b - 15):b + 15]
    if c0 != c1:
        return "comments differ"
    if p0 != p1:
        return "preprocessor lines differ"
    if m0 != m1:
        return "multi-line string differs"
    lost = strings_kept(strings0, new)
    if lost is not None:
        return "string literal differs: %r" % lost[:40]
    return None


def code_equal(a, b, spec, lang):
    """Like same_code, but ignoring comments entirely (gofmt/gjf rewrap and renumber them)."""
    sa = lex_view(a, spec, lang, comment_chars=False)
    sb = lex_view(b, spec, lang, comment_chars=False)
    strip = lambda x: re.sub(r",(?=\|?[)\]}>])", "", x)  # noqa: E731
    return strip(sa[0]) == strip(sb[0]) and sa[4] == sb[4] and sa[5] == sb[5]


def same_code(a, b, spec, lang):
    """Formatter round-trip comparison: same code tokens, same comment *words* (formatters
    may re-wrap long comments), same preprocessor lines and multi-line strings."""
    sa, _, _, ca, pa, ma, _ = lex_view(a, spec, lang, comment_chars=False)
    sb, _, _, cb, pb, mb, _ = lex_view(b, spec, lang, comment_chars=False)

    def words(cs):
        return [w for c in cs for w in re.sub(r"^\s*(//+|/\*+|\*+/?|#)", " ", c).split()
                if w not in ("//", "/*", "*/", "*", "#")]
    def no_trailing_commas(x):  # formatters add/remove trailing commas with the layout
        return re.sub(r",(?=\|?[)\]}>])", "", x)
    return no_trailing_commas(sa) == no_trailing_commas(sb) and words(ca) == words(cb) \
        and pa == pb and ma == mb


def lex_signature(code, spec, lang, strip_comments=False):
    """Hashable summary: equal for two texts that differ only in layout."""
    s, _, _, c, p, m, _ = lex_view(code, spec, lang, strip_comments)
    return (s, tuple(c), tuple(p), tuple(m))


_TS = None


def _ts_parser(name):
    global _TS
    if _TS is None and os.environ.get("COMPACT_CODE_NO_AST"):
        _TS = False  # e.g. to test the behaviour without tree-sitter
    if _TS is None:
        try:
            import tree_sitter_language_pack as tslp  # optional
            _TS = tslp
        except Exception:
            _TS = False
    if not _TS or not name:
        return None
    try:
        return _TS.get_parser(name)
    except Exception:
        return None


AUTO_SEMI = {";", "\n", ""}
AST_WRAPPERS = {"expression_statement"}
AST_SKIP = {"empty_statement"}  # `};` after a block is a harmless empty statement
# Only statement-level structure is compared (that is what ASI / semicolon insertion can
# change); expression-level labels such as call vs. type conversion are ambiguous in GLR
# grammars and would cause false alarms. Leaves (the tokens) are always compared.
STRUCT_RE = re.compile(r"statement|declaration|definition|block|body|clause|program|source_file|"
                       r"module|class|function|method|lambda|arrow|jsx|element|case|return|"
                       r"export|import|namespace|interface|enum|struct|impl|trait|object")


def jsx_text_value(txt):
    """The string React/Babel actually produces for a JSX text child."""
    lines = txt.replace("\r", "").split("\n")
    if len(lines) == 1:
        return txt
    parts = []
    for i, ln in enumerate(lines):
        if i > 0:
            ln = ln.lstrip(" \t")
        if i < len(lines) - 1:
            ln = ln.rstrip(" \t")
        if ln:
            parts.append(ln)
    return " ".join(parts)  # grammars wrap the same construct inconsistently


def ast_signature(code, ts_name, strip_comments=False):
    """Bracketed pre-order serialization of the tree-sitter parse (whitespace-free)."""
    parser = _ts_parser(ts_name)
    if parser is None:
        return None
    tree = parser.parse(code.encode("utf-8", "surrogateescape"))
    out, bad = [], False
    stack = [tree.root_node]
    CLOSE = object()
    while stack:
        node = stack.pop()
        if node is CLOSE:
            out.append((">",))
            continue
        typ = node.type
        if node.is_missing or typ == "ERROR":
            bad = True
        if typ in AST_SKIP:
            continue
        if "comment" in typ:
            if not strip_comments:
                out.append(("c", node.text.decode("utf-8", "replace").rstrip()))
            continue
        if node.child_count == 0:
            txt = node.text.decode("utf-8", "replace")
            if node.is_named and "jsx_text" in typ:
                val = jsx_text_value(txt)
                if val:
                    out.append(("T", val))
            elif node.is_named and "text" in typ:
                out.append(("T", tuple(txt.split())))
            elif txt.strip() not in AUTO_SEMI:
                out.append(("L", txt))  # leaf labels (identifier vs package_identifier) are ambiguous
            continue
        if node.is_named and typ not in AST_WRAPPERS and STRUCT_RE.search(typ):
            out.append(("<", typ))
            stack.append(CLOSE)
        stack.extend(reversed(node.children))
    return out, bad


def verify(orig, new, spec, lang, strip_comments):
    """Return (ok, how). Lexical check always; AST check when tree-sitter is available."""
    try:
        if lexical_diff(orig, new, spec, lang, strip_comments):
            return False, "lex"
    except LexFail:
        return False, "lex"
    a = ast_signature(orig, spec.get("ts_verify", spec.get("ts")), strip_comments)
    if a is None:
        return True, "lex"
    sig_a, bad_a = a
    if bad_a:
        return True, "lex"  # grammar can't parse the original: AST check not meaningful
    sig_b, bad_b = ast_signature(new, spec.get("ts_verify", spec.get("ts")), strip_comments)
    if bad_b and spec["join"] in ("go", "kotlin"):
        # tree-sitter-go and tree-sitter-kotlin reject some valid one-line code (Go var
        # groups, ...); their newlines are checked lexically, so this is noise, not evidence
        return True, "lex"
    if bad_b or sig_a != sig_b:
        return False, "ast"
    return True, "lex+ast"


# ----------------------------------------------------------------- driver
def compact_text(code, lang, level=3, strip_comments=False, outline=None, wrap=0,
                 numbers=False, check=True, use_ast=True, paragraphs=False):
    """Compact `code`. Returns (text, info dict)."""
    spec = LANGS[lang]
    code = code.lstrip("\ufeff").replace("\r\n", "\n").replace("\r", "\n")
    if code and not code.endswith("\n"):
        code += "\n"  # lexers only recognise a final `// comment` when a newline ends it
    want = LEVELS[level]
    flags = set(want & allowed_flags(lang, code))
    if paragraphs and flags & {"blank", "indent", "join"}:
        flags.add("para")
    info = {"lang": lang, "requested": level, "flags": set(flags), "verified": "-",
            "fallback": []}
    if not flags and not strip_comments:
        return code, info
    lang = refine_language(lang, code)  # .h -> C++/ObjC, .js with JSX -> jsx
    spec = LANGS[lang]
    info["lang"] = lang
    try:
        lexer = make_lexer(spec, code)
    except Exception as e:  # unknown lexer
        info["error"] = str(e)
        return code, info
    global _TS, ALLOW_LEX_ERRORS, JS_SEMI
    if not use_ast:
        _TS = False
    ALLOW_LEX_ERRORS = check and _ts_parser(spec.get("ts_verify")) is not None
    # ';' for ASI newlines only when the AST check can actually vouch for it on this file
    JS_SEMI = ALLOW_LEX_ERRORS and not (ast_signature(code, spec.get("ts_verify")) or ((), True))[1]
    while True:
        try:
            text = render(code, spec, lexer, flags, strip_comments, outline, wrap)
            if not check:
                info["flags"] = set(flags)
                break
            ok, how = verify(code, text, spec, lang, strip_comments)
        except LexFail as e:
            info["error"] = str(e)
            return code, info
        if ok:
            info["flags"], info["verified"] = set(flags), how
            break
        drop = next((f for f in FLAG_ORDER if f in flags), None)
        if drop == "blank":
            flags.discard("para")
        info["fallback"].append("%s:%s" % (drop, how))
        if drop is None:
            return code, info
        flags.discard(drop)
        if not flags - {"para"} and not strip_comments:
            info["flags"] = set()
            return code, info
    if numbers:
        text = render(code, spec, lexer, flags, strip_comments, outline, wrap, numbers=True)
    return text, info


def level_of(flags):
    for lv in (4, 3, 2, 1):
        if LEVELS[lv] <= flags:
            return lv
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Strip non-semantic formatting from source code (token-lean view).",
        formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__.split("\n\n", 1)[1])
    ap.add_argument("paths", nargs="*", help="files, directories, or - for stdin")
    ap.add_argument("-l", "--lang", help="force language (required for stdin)")
    ap.add_argument("-L", "--level", type=int, default=3, choices=range(5))
    ap.add_argument("-p", "--paragraphs", action="store_true",
                    help="keep one blank line where the source had blank lines (about 1 token each); "
                         "lets formatters restore the original vertical spacing exactly")
    ap.add_argument("-c", "--strip-comments", action="store_true",
                    help="also drop comments (directive comments like //go:build are kept)")
    ap.add_argument("--outline", type=int, metavar="DEPTH",
                    help="keep line breaks at brace depth <= DEPTH (0 = one top-level item per "
                         "line, 1 = one member per line)")
    ap.add_argument("-n", "--numbers", action="store_true",
                    help="prefix each output line with its original line number (read-only view)")
    ap.add_argument("-w", "--wrap", type=int, default=1500,
                    help="soft-wrap long lines at an original line break (0 = never; default 1500)")
    ap.add_argument("-o", "--output", help="write the combined output to this file")
    ap.add_argument("-q", "--quiet", action="store_true", help="no per-file summary on stderr")
    ap.add_argument("--no-verify", action="store_true", help="skip the re-lex safety check")
    ap.add_argument("--no-ast", action="store_true", help="skip the tree-sitter AST check")
    ap.add_argument("--list-languages", action="store_true")
    a = ap.parse_args(argv)

    if a.list_languages:
        names = {0: "as-is", 1: "blank", 2: "lines", 3: "flat", 4: "dense"}
        for k in sorted(LANGS):
            s = LANGS[k]
            print("%-12s max L%d (%s)%s  %s" % (k, level_of(s["allowed"]), names[level_of(s["allowed"])],
                                               "  indent-sensitive" if s["indent_sensitive"] else "",
                                               " ".join(s["exts"] + s["names"])))
        return 0
    if not a.paths:
        ap.print_usage(sys.stderr)
        return 1

    files = []
    for p in a.paths:
        if p == "-":
            files.append("-")
        elif os.path.isdir(p):
            files.extend(iter_source_files(p))
        else:
            files.append(p)
    multi = len(files) > 1
    chunks, rc = [], 0
    for path in files:
        lang = resolve_lang(a.lang) if a.lang else (None if path == "-" else detect_language(path))
        try:
            if path == "-":
                code = sys.stdin.buffer.read().decode("utf-8", "surrogateescape")
            else:
                with open(path, "rb") as fh:
                    code = fh.read().decode("utf-8", "surrogateescape")
        except OSError as e:
            sys.stderr.write("compact.py: %s\n" % e)
            rc = 1
            continue
        if "\0" in code:
            sys.stderr.write("compact.py: skipping binary file %s\n" % path)
            continue
        if lang is None:
            if path == "-" or not multi:
                sys.stderr.write("compact.py: unknown language for %s (use -l)\n" % path)
            text, info = code, {"lang": "?", "flags": set(), "verified": "-", "fallback": []}
        else:
            text, info = compact_text(code, lang, a.level, a.strip_comments, a.outline, a.wrap,
                                      a.numbers, check=not a.no_verify, use_ast=not a.no_ast,
                                      paragraphs=a.paragraphs)
        if not a.quiet:
            before, after = len(code), len(text)
            pct = 100.0 * (after - before) / before if before else 0.0
            extra = ""
            if info.get("fallback"):
                extra += " fallback[%s]" % ",".join(info["fallback"])
            if info.get("error"):
                extra += " kept-as-is(%s)" % info["error"]
            sys.stderr.write("compact: %s %s L%d %d->%d chars (%+.1f%%) verify=%s%s\n" % (
                path, info["lang"], level_of(info["flags"]), before, after, pct,
                info["verified"], extra))
        chunks.append(("==> %s <==\n" % path if multi else "") + text)
    result = "\n".join(chunks)
    if a.output:
        with open(a.output, "wb") as fh:
            fh.write(result.encode("utf-8", "surrogateescape"))
    else:
        sys.stdout.buffer.write(result.encode("utf-8", "surrogateescape"))
        sys.stdout.flush()
    return rc


if __name__ == "__main__":
    sys.exit(main())
