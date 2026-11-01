# Generic exec recipes (the exec template)

Part of `devcontainer:workflow` (`../SKILL.md`): read it when you need a command for a fast signal, the full suite, dependency resolution, artifact inspection or a registry query — and the note on verbosity flags.

Generic recipes (substitute the project's actual build tool — `mvn`,
`npm`/`yarn`/`pnpm`, `pytest`/`tox`, `cargo`, `go test`, etc. — and use
the `devcontainer exec` form whenever the CLI is present):

```bash
# Fast signal after an edit — whatever the lightest "does this parse/compile" step is
devcontainer exec --workspace-folder <path> <build-tool> <compile-or-build-check>
# fallback: $CONTAINER_ENGINE exec -u <user> --workdir <path> <container> <build-tool> <compile-or-build-check>

# Full test suite
devcontainer exec --workspace-folder <path> <build-tool> test
# fallback: $CONTAINER_ENGINE exec -u <user> --workdir <path> <container> <build-tool> test

# Inspect exactly what versions/deps actually resolved (the equivalent of
# `mvn dependency:tree`, `npm ls <pkg>`, `pip show`, `cargo tree`, etc.) —
# this is how classpath/version-conflict bugs get diagnosed, and it beats
# guessing from the manifest file alone, since transitive resolution can
# silently override what the top-level manifest says
devcontainer exec --workspace-folder <path> <dependency-tree-equivalent>

# Look inside a built artifact for what actually got embedded
devcontainer exec --workspace-folder <path> <artifact-inspection-command>

# Query the package registry directly from inside the container when you
# need to know what versions exist, or what a dependency's own manifest
# requires transitively
devcontainer exec --workspace-folder <path> curl -s <registry-metadata-url>
```

Watch out for verbosity flags that suppress the exact output you need:
e.g. Maven's `dependency:tree` logs at INFO through the Maven logger, so a
blanket `-q` silences it entirely. Drop quiet/silent flags for any command
whose output you actually need to read; keep them only for high-volume,
low-signal steps where you mainly care about the exit code and the tail of
any failure.
