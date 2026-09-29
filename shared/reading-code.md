# Reading code without wasting tokens

For skills that read a project's code in bulk: surveys, reverse
engineering, reviews, audits, and the reading before a build or an
upgrade. The Read tool puts every line in context with its line number,
which by itself adds 20–45% to a file's tokens.

1. **Map before reading.** Layout, manifests and entry points first; then
   `grep -rn` / Glob for the symbols in question; read a file only once you
   know you need it, and only the part you need.
2. **Large or many files in a brace language** (Java, C#, C/C++, Go, Rust,
   JS/TS, Kotlin, PHP, Dart, Swift, CSS, JSON…): read them through
   `compact:code` — a verified compact view, 29–39% cheaper than a
   line-numbered Read in testing, 42–48% with comments dropped
   (`-c`) when only the logic matters. `--outline 1 -n` gives one member per
   line with the original line numbers, to pick what to read closely.
3. **Other files** (Python, YAML, shell, Markdown, SQL, config): `sed -n
   'a,bp'` or `cat` for a range or a whole file — no line-number overhead —
   and Read with an offset and limit when you need exact lines.
4. **Never a whole-file Read of a big file** just to find something in it.
5. **Before an edit, Read the exact lines** from the file on disk (offset
   and limit). Edit text never comes from a compact or reformatted view.
6. **Show the user real code**, never the compact view.
