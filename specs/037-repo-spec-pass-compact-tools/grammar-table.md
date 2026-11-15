# Grammar table (spec 037 T060)

Research of 2026-11-02 for the 36 tree-sitter names `langs.py` references. Source of crate data: the crates.io API (version, license, publish date, dependency requirements of the latest stable release). Source of sizes: `rust/spikes/compact` (a standalone crate with one cargo feature per grammar), `cargo build --release` with the workspace profile (`opt-level = "z"`, LTO, strip), x86_64 Linux glibc; each size is the growth of a probe binary (baseline 303,360 bytes: `tree-sitter` core plus a stub) that links that one grammar and parses an empty file. Sizes are therefore an upper bound on what the grammar costs inside the real `ai-gent` (shared code is counted once there), and they differ per target.

## How the verdict is decided

- `yes`: a maintained crate exists, it declares `tree-sitter-language` (the stable ABI shim) as its only normal dependency on the core, so it works with any `tree-sitter` runtime, the license is in `rust/deny.toml`, and the probe loaded the grammar and parsed an empty file in the spike build.
- `no`: no crate that satisfies the above.
- `unknown`: a crate exists but a check could not be made.
- Every one of the 36 loaded in the probe. The verdict says nothing about whether compact can reach L4 with that grammar; that needs a renderer and a verifier per language (T062).

| # | tree-sitter name | Crate | Version | Core requirement | License | Published | Added to a release binary | In the 24 L4 set | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| 1 | bash | `tree-sitter-bash` | 0.25.1 | tree-sitter ^0.25 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-12-02 | 1.38 MB | no | yes |
| 2 | c | `tree-sitter-c` | 0.24.2 | tree-sitter ^0.25.4 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-04-22 | 0.66 MB | yes | yes |
| 3 | cpp | `tree-sitter-cpp` | 0.23.4 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-11 | 3.47 MB | yes | yes |
| 4 | csharp | `tree-sitter-c-sharp` | 0.23.5 | tree-sitter ^0.25 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-04-14 | 5.37 MB | yes | yes |
| 5 | css | `tree-sitter-css` | 0.25.0 | tree-sitter ^0.25 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-09-28 | 0.17 MB | yes | yes |
| 6 | cuda | `tree-sitter-cuda` | 0.21.2 | tree-sitter ^0.27.0 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-09-15 | 7.08 MB | yes | yes |
| 7 | dart | `tree-sitter-dart` | 0.3.0 | tree-sitter ^0.26 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-10-25 | 1.51 MB | yes | yes |
| 8 | elixir | `tree-sitter-elixir` | 0.3.5 | tree-sitter ^0.23.0 (dev) ; tree-sitter-language ^0.1 | Apache-2.0 | 2026-03-02 | 1.45 MB | no | yes |
| 9 | glsl | `tree-sitter-glsl` | 0.2.0 | tree-sitter ^0.25.3 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-03-16 | 0.89 MB | yes | yes |
| 10 | go | `tree-sitter-go` | 0.25.0 | tree-sitter ^0.25.8 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-08-29 | 0.27 MB | yes | yes |
| 11 | groovy | `tree-sitter-groovy` | 0.1.2 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-19 | 1.05 MB | no | yes |
| 12 | haskell | `tree-sitter-haskell` | 0.24.1 | tree-sitter ^0.27.0 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-09-30 | 3.90 MB | no | yes |
| 13 | hlsl | `tree-sitter-hlsl` | 0.2.0 | tree-sitter ^0.25.3 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-03-23 | 4.26 MB | yes | yes |
| 14 | html | `tree-sitter-html` | 0.23.2 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-11 | 0.07 MB | no | yes |
| 15 | java | `tree-sitter-java` | 0.23.5 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-12-21 | 0.47 MB | yes | yes |
| 16 | javascript | `tree-sitter-javascript` | 0.25.0 | tree-sitter ^0.25.8 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-09-01 | 0.46 MB | yes | yes |
| 17 | json | `tree-sitter-json` | 0.24.8 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-11 | 0.05 MB | yes | yes |
| 18 | kotlin | `tree-sitter-kotlin-ng` | 1.1.0 | tree-sitter ^0.24 (dev) (kotlin-ng; the older `tree-sitter-kotlin` 0.3.8 pins tree-sitter <0.23) | MIT | 2025-01-09 | 3.47 MB | yes | yes |
| 19 | lua | `tree-sitter-lua` | 0.5.0 | tree-sitter ^0.26.3 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-02-26 | 0.10 MB | no | yes |
| 20 | objc | `tree-sitter-objc` | 3.0.2 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-12-16 | 5.34 MB | yes | yes |
| 21 | php | `tree-sitter-php` | 0.25.1 | tree-sitter ^0.27 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-10-06 | 1.13 MB | yes | yes |
| 22 | python | `tree-sitter-python` | 0.25.0 | tree-sitter ^0.25.8 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-09-11 | 0.51 MB | no | yes |
| 23 | ruby | `tree-sitter-ruby` | 0.23.1 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-11 | 2.15 MB | no | yes |
| 24 | rust | `tree-sitter-rust` | 0.24.2 | tree-sitter ^0.25 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-03-27 | 1.16 MB | yes | yes |
| 25 | scala | `tree-sitter-scala` | 0.26.4 | tree-sitter ^0.26 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-11-01 | 4.10 MB | no | yes |
| 26 | scss | `tree-sitter-scss` | 1.0.0 | tree-sitter >=0.21.0 (normal; the old `language()` API, no tree-sitter-language) | MIT | 2024-04-09 | 0.19 MB | yes | yes |
| 27 | solidity | `tree-sitter-solidity` | 1.2.13 | tree-sitter ^0.25 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-08-04 | 0.57 MB | yes | yes |
| 28 | sql | `tree-sitter-sequel` | 0.3.11 | tree-sitter ~0.25.0 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-10-01 | 2.50 MB | no | yes |
| 29 | swift | `tree-sitter-swift` | 0.7.4 | tree-sitter ^0.23.0 (dev) ; tree-sitter-language ^0.1 | MIT | 2026-10-04 | 4.17 MB | no | yes |
| 30 | toml | `tree-sitter-toml-ng` | 0.7.0 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-12-03 | 0.07 MB | no | yes |
| 31 | tsx | `tree-sitter-typescript` | 0.23.2 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-11 | 2.91 MB (shared with typescript) | yes | yes |
| 32 | typescript | `tree-sitter-typescript` | 0.23.2 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-11 | 2.91 MB | yes | yes |
| 33 | verilog | `tree-sitter-verilog` | 1.0.3 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-10 | 18.17 MB | no | yes |
| 34 | xml | `tree-sitter-xml` | 0.7.0 | tree-sitter ^0.24 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-11-13 | 0.10 MB | no | yes |
| 35 | yaml | `tree-sitter-yaml` | 0.7.2 | tree-sitter ^0.25.4 (dev) ; tree-sitter-language ^0.1 | MIT | 2025-10-07 | 0.24 MB | no | yes |
| 36 | zig | `tree-sitter-zig` | 1.1.2 | tree-sitter ^0.24.5 (dev) ; tree-sitter-language ^0.1 | MIT | 2024-12-22 | 0.74 MB | no | yes |

Crate notes:
- `kotlin`: `tree-sitter-kotlin-ng` 1.1.0 (the maintained fork); the original `tree-sitter-kotlin` is last published 2024-08 and requires `tree-sitter` below 0.23.
- `sql`: `tree-sitter-sequel` 0.3.11; `tree-sitter-sql` 0.0.2 is from 2021 (tree-sitter 0.19) and is not usable.
- `toml`: `tree-sitter-toml-ng` 0.7.0; `tree-sitter-toml` is from 2022.
- `dart`: `tree-sitter-dart` 0.3.0 (published 2026-10-25); `tree-sitter-dart-orchard` 0.7.1 is an alternative.
- `scss`: `tree-sitter-scss` 1.0.0 (2024-04) uses the pre-`tree-sitter-language` API (`language()` returning a `tree_sitter::Language`); it linked and parsed against `tree-sitter` 0.25 in the spike, but it is the one grammar here that ties to the core version, so a core upgrade can break it.
- `tsx` and `typescript` are one crate (`tree-sitter-typescript`, two languages); `jsx` is parsed by `tree-sitter-javascript`.
- `zig`: `tree-sitter-zig` 1.1.2 (2024-12), its `build` dependency is `cc` (as the others); the crates.io API lookup for the table failed once on rate limiting, the data above was fetched again by hand.
- Several grammars declare `tree-sitter` only as a *dev* dependency (`bash`, `c`, `cpp`, ...): the version there is what the grammar was tested with, not a constraint on the core.

## Entries of `langs.py` with no tree-sitter name (19)

clojure, coffeescript, dockerfile, elm, fsharp, less, make, markdown, nim, objcpp, perl, powershell, proto, pug, r, rst, sass, stylus, terraform (from the plan). Of these `objcpp`, `proto` and `less` are L4 today and drop to L1 without a grammar. `objcpp` could reuse the `objc` grammar only if it parses Objective-C++ (not tested).

## Totals

- 36 of 36 names have a usable crate and loaded in the spike build.
- Linking all of them: +78,515,760 bytes over the 303,360-byte baseline (the binary becomes about 78.8 MB).
- The 20 grammar-backed members of the L4 set (with `jsx` sharing javascript, 21 languages): see `measurements.md`, section Compact spike, for the sums against the 8 MB binary budget of spec 032.
