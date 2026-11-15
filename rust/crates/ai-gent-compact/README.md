# ai-gent-compact

Token-lean source code reader of [ai-gent](https://github.com/lucasvmigotto/ai-gent): a compact
view of a source file (indentation and optional spaces removed, strings and comments kept,
re-parsed and verified before it is printed) and an outline with the original line numbers.
It ships as its own binary, `ai-gent-compact`, with its own size budget, apart from the core
`ai-gent` binary (decision D29).

```
ai-gent-compact -L4 src/lib.rs            # one file, densest level
ai-gent-compact --outline 1 -n src/       # a directory, one member per line, original line numbers
ai-gent-compact --list-languages
```

Compacted: Java, C#, C, C++, Go, Rust, JavaScript, TypeScript, TSX, Kotlin, CSS (SCSS to level 2).
Any other language is printed as it is with a note, "outline only, no compaction". Read-only: the
only file written is the one named by `-o`.

Project: <https://github.com/lucasvmigotto/ai-gent>.
