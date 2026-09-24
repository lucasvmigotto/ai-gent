# Worked example: diagnosing a runtime/classpath failure

Part of `devcontainer:workflow` (`../SKILL.md`), which has the exec
template, the file-vs-environment split and the rebuild procedure.

This is the general shape of a real fix done this way — useful as a
template for the next dependency/runtime bug in any language:

1. A build/run command fails with a runtime error (missing class, module
   not found, symbol mismatch, etc.). Read the **full** error/stack
   trace — the first "caused by" line is rarely the interesting one; the
   deepest cause usually is.
2. Use the project's dependency-tree-equivalent (in the container) to see
   what versions **actually** resolved, not just what the top-level
   manifest declares — transitive dependency management can silently
   override it.
3. If two dependencies need incompatible versions of something they share
   transitively, check what version each one's own manifest expects
   (read it from the container's local package cache, or fetch it from
   the registry) rather than guessing at a fix.
4. Make the fix with `Edit` on the host manifest file — never inside the
   container.
5. Re-run the dependency-tree-equivalent (container) to confirm the
   resolved versions now match expectations, then run the test suite and
   actually start the app (backgrounded, read via `TaskOutput`) to confirm
   it boots — a successful compile only proves the code compiles, not that
   the dependency graph is runtime-consistent.
6. Diff the host manifest (`git diff`) to show the user exactly what
   changed, and leave a short comment in the manifest explaining any
   non-obvious version constraint — future readers won't know why a
   version looks "downgraded" otherwise.
