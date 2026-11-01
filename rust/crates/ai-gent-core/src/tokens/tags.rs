//! The tag vocabulary (`shared/token-tags.json`, embedded), tag parsing and formatting, tags
//! inferred for untagged work, and the repository's languages and stack read from its manifests.
use super::pyjson::{self, J};
use super::util::{OMap, glob_children, is_word, splitlines};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

/// A card's tags: facet to a string, or a list of strings for a multi-valued facet. Ordered.
pub type Tags = Vec<(String, J)>;

/// The facet's value in `tags`.
#[must_use]
pub fn tag_get<'a>(tags: &'a Tags, facet: &str) -> Option<&'a J> {
    tags.iter().find(|(k, _)| k == facet).map(|(_, v)| v)
}

/// Set a facet, keeping its position when it exists.
pub fn tag_set(tags: &mut Tags, facet: &str, v: J) {
    if let Some(slot) = tags.iter_mut().find(|(k, _)| k == facet) {
        slot.1 = v;
    } else {
        tags.push((facet.to_string(), v));
    }
}

/// One facet of the vocabulary.
pub struct Facet {
    /// The facet name.
    pub name: String,
    /// More than one value allowed.
    pub multi: bool,
    /// value -> description (for `stack`: value -> the language it belongs to).
    pub values: OMap<J>,
    /// alias -> value (only `stack` has them).
    pub aliases: OMap<String>,
}

const VOCAB_JSON: &str = include_str!("token-tags.json");

/// The facets in file order.
///
/// # Panics
/// Never in practice: the embedded file is checked by `scripts/consistency.py` and the tests.
#[must_use]
pub fn vocab() -> &'static [Facet] {
    static V: OnceLock<Vec<Facet>> = OnceLock::new();
    V.get_or_init(|| {
        let doc = pyjson::parse(VOCAB_JSON).expect("embedded token-tags.json is valid");
        let Some(J::Obj(facets)) = doc.get("facets") else {
            panic!("token-tags.json has no facets")
        };
        facets
            .iter()
            .map(|(name, f)| {
                let mut values = OMap::new();
                if let Some(J::Obj(vs)) = f.get("values") {
                    for (k, v) in vs {
                        values.set(k, v.clone());
                    }
                }
                let mut aliases = OMap::new();
                if let Some(J::Obj(al)) = f.get("aliases") {
                    for (k, v) in al {
                        aliases.set(k, v.as_str().unwrap_or_default().to_string());
                    }
                }
                Facet {
                    name: name.clone(),
                    multi: matches!(f.get("multi"), Some(J::Bool(true))),
                    values,
                    aliases,
                }
            })
            .collect()
    })
}

fn facet(name: &str) -> Option<&'static Facet> {
    vocab().iter().find(|f| f.name == name)
}

/// A package or crate name as the vocabulary's stack value.
#[must_use]
pub fn norm_stack(name: &str) -> Option<String> {
    let st = facet("stack")?;
    let n = name.trim().to_lowercase();
    for cand in [n.clone(), n.replace('_', "-")] {
        if st.values.contains_key(&cand) {
            return Some(cand);
        }
        if let Some(a) = st.aliases.get(&cand) {
            return Some(a.clone());
        }
    }
    None
}

/// `'extend, backend, rust, tokio, M'` to tags; unknown words are ignored.
#[must_use]
pub fn parse_tags(text: &str) -> Tags {
    let mut tags: Tags = Vec::new();
    for word in text.split(',').map(str::trim).filter(|w| !w.is_empty()) {
        for f in vocab() {
            let value = if f.values.contains_key(word) {
                Some(word.to_string())
            } else if f.name == "stack" {
                norm_stack(word)
            } else {
                None
            };
            let Some(value) = value else { continue };
            if f.multi {
                let mut set: BTreeSet<String> = match tag_get(&tags, &f.name) {
                    Some(J::Arr(a)) => a
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect(),
                    _ => BTreeSet::new(),
                };
                set.insert(value);
                tag_set(
                    &mut tags,
                    &f.name,
                    J::Arr(set.into_iter().map(J::Str).collect()),
                );
            } else if tag_get(&tags, &f.name).is_none() {
                tag_set(&mut tags, &f.name, J::Str(value));
            }
            break;
        }
    }
    tags
}

/// Tags back to `'extend, backend, rust, react, tokio, M'` in vocabulary order.
#[must_use]
pub fn format_tags(tags: &Tags) -> String {
    let mut out: Vec<String> = Vec::new();
    for f in vocab() {
        match tag_get(tags, &f.name) {
            Some(J::Arr(a)) => out.extend(a.iter().filter_map(|x| x.as_str().map(String::from))),
            Some(J::Str(s)) if !s.is_empty() => out.push(s.clone()),
            _ => {}
        }
    }
    out.join(", ")
}

/// `S`, `M` or `L` from a task count.
#[must_use]
pub fn size_of(n: Option<usize>) -> Option<&'static str> {
    match n {
        None | Some(0) => None,
        Some(1..=3) => Some("S"),
        Some(4..=6) => Some("M"),
        Some(_) => Some("L"),
    }
}

// ------------------------------------------------------------ repository

/// The repository's languages and stack.
#[derive(Debug, Clone, Default)]
pub struct RepoInfo {
    /// The first language found.
    pub lang: Option<String>,
    /// All languages, in discovery order.
    pub langs: Vec<String>,
    /// Normalized stack values, sorted.
    pub stack: Vec<String>,
}

const MANIFESTS: [&str; 6] = [
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
];

/// `re.match(r"(?:\w+-)?dependencies\.(.+)", section)`.
fn dep_sub(section: &str) -> Option<&str> {
    let w = section
        .char_indices()
        .find(|&(_, c)| !is_word(c))
        .map_or(section.len(), |(i, _)| i);
    let cand = if w > 0 && section[w..].starts_with('-') {
        &section[w + 1..]
    } else {
        section
    };
    cand.strip_prefix("dependencies.")
        .or_else(|| section.strip_prefix("dependencies."))
        .filter(|n| !n.is_empty())
}

fn cargo_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut section = String::new();
    for line in splitlines(text) {
        let t = line.trim_start();
        let header = t.strip_prefix('[').and_then(|r| {
            let chars: Vec<char> = r.chars().collect();
            (1..chars.len())
                .find(|&i| chars[i] == ']')
                .map(|i| chars[..i].iter().collect::<String>())
        });
        if let Some(h) = header {
            section = h;
            if let Some(name) = dep_sub(&section) {
                names.push(name.to_string());
            }
        } else if section.contains("dependencies") {
            let name: String = t
                .chars()
                .take_while(|&c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                .collect();
            if !name.is_empty() && t[name.len()..].trim_start().starts_with('=') {
                names.push(name);
            }
        }
    }
    names
}

fn pyproject_names(text: &str) -> Vec<String> {
    let c: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if (c[i] == '"' || c[i] == '\'') && c.get(i + 1).is_some_and(char::is_ascii_alphabetic) {
            let mut j = i + 1;
            while j < c.len() && (c[j].is_ascii_alphanumeric() || matches!(c[j], '_' | '.' | '-')) {
                j += 1;
            }
            let name: String = c[i + 1..j].iter().collect();
            let mut k = j;
            while k < c.len() && c[k].is_whitespace() {
                k += 1;
            }
            if c.get(k).is_some_and(|x| "<>=!~[;\"'".contains(*x)) {
                out.push(name);
                i = k + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn go_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut t = line.trim_start();
        if let Some(r) = t.strip_prefix("require") {
            if r.starts_with(char::is_whitespace) {
                t = r.trim_start();
            }
        }
        let run: String = t
            .chars()
            .take_while(|&c| is_word(c) || matches!(c, '.' | '-' | '/'))
            .collect();
        let Some(slash) = run.find('/') else { continue };
        if slash == 0
            || slash + 1 >= run.len()
            || !run[..slash]
                .chars()
                .all(|c| is_word(c) || matches!(c, '.' | '-'))
        {
            continue;
        }
        let rest = &t[run.len()..];
        let trimmed = rest.trim_start_matches(|c: char| c.is_whitespace() && c != '\n');
        if trimmed.len() < rest.len() && trimmed.starts_with('v') {
            out.push(run);
        }
    }
    out
}

/// The repository's languages and stack, read from its manifests at the root and one folder down.
/// Read-only.
#[must_use]
pub fn repo_info(repo_dir: &Path) -> RepoInfo {
    if !repo_dir.is_dir() {
        return RepoInfo::default();
    }
    let mut paths: Vec<std::path::PathBuf> = MANIFESTS.iter().map(|m| repo_dir.join(m)).collect();
    for sub in glob_children(repo_dir) {
        let base = sub
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if sub.is_dir()
            && !["node_modules", "target", "dist", ".git", ".worktrees"].contains(&base.as_str())
        {
            paths.extend(MANIFESTS.iter().map(|m| sub.join(m)));
        }
    }
    let (mut langs, mut stack): (Vec<String>, BTreeSet<String>) = (Vec::new(), BTreeSet::new());
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes)
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let base = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (lang, names): (&str, Vec<String>) = match base.as_str() {
            "Cargo.toml" => ("rust", cargo_names(&text)),
            "package.json" => {
                let mut names = Vec::new();
                if let Some(pkg) = pyjson::parse(&text) {
                    for key in ["dependencies", "devDependencies"] {
                        if let Some(J::Obj(m)) = pkg.get(key) {
                            names.extend(m.iter().map(|(k, _)| k.clone()));
                        }
                    }
                }
                let ts = names.iter().any(|n| n == "typescript")
                    || path
                        .parent()
                        .is_some_and(|p| p.join("tsconfig.json").exists());
                (if ts { "typescript" } else { "javascript" }, names)
            }
            "pyproject.toml" => ("python", pyproject_names(&text)),
            "go.mod" => ("go", go_names(&text)),
            _ => (
                "java",
                if text.contains("spring-boot") {
                    vec!["spring-boot".to_string()]
                } else {
                    vec![]
                },
            ),
        };
        if !langs.iter().any(|l| l == lang) {
            langs.push(lang.to_string());
        }
        stack.extend(names.iter().filter_map(|n| norm_stack(n)));
    }
    RepoInfo {
        lang: langs.first().cloned(),
        langs,
        stack: stack.into_iter().collect(),
    }
}

// ------------------------------------------------------------- inference

fn ext_lang(ext: &str) -> Option<&'static str> {
    Some(match ext {
        ".rs" => "rust",
        ".ts" | ".tsx" => "typescript",
        ".js" | ".jsx" | ".mjs" | ".cjs" => "javascript",
        ".py" => "python",
        ".go" => "go",
        ".java" => "java",
        ".kt" => "kotlin",
        ".cs" => "csharp",
        ".php" => "php",
        ".rb" => "ruby",
        ".sh" => "sh",
        ".sql" => "sql",
        ".c" | ".h" | ".cc" | ".cpp" | ".hpp" => "cpp",
        _ => return None,
    })
}

/// `os.path.splitext(f)[1].lower()`.
fn extension(f: &str) -> String {
    let base = f.rsplit(['/', '\\']).next().unwrap_or(f);
    match base.rfind('.') {
        Some(i) if i > 0 && base[..i].chars().any(|c| c != '.') => base[i..].to_lowercase(),
        _ => String::new(),
    }
}

fn agent_lang(files: &[String], repo: Option<&RepoInfo>) -> Option<String> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for f in files {
        if let Some(l) = ext_lang(&extension(f)) {
            match counts.iter_mut().find(|(n, _)| *n == l) {
                Some(c) => c.1 += 1,
                None => counts.push((l, 1)),
            }
        }
    }
    if !counts.is_empty() {
        let mut best = counts[0];
        for c in &counts[1..] {
            if c.1 > best.1 {
                best = *c;
            }
        }
        return Some(best.0.to_string());
    }
    if !files.is_empty() && files.iter().all(|f| f.to_lowercase().ends_with(".md")) {
        return Some("markdown".into());
    }
    repo.and_then(|r| r.lang.clone())
}

/// Word runs (`\w+`) of a lowercased text with their char positions.
fn runs(d: &str) -> Vec<(usize, usize, String)> {
    let c: Vec<char> = d.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if is_word(c[i]) {
            let s = i;
            while i < c.len() && is_word(c[i]) {
                i += 1;
            }
            out.push((s, i, c[s..i].iter().collect()));
        } else {
            i += 1;
        }
    }
    out
}

fn has_any(r: &[(usize, usize, String)], words: &[&str]) -> bool {
    r.iter().any(|(_, _, w)| words.contains(&w.as_str()))
}

/// Two runs separated by exactly one space: `first second`.
fn has_pair(d: &str, r: &[(usize, usize, String)], first: &[&str], second: &[&str]) -> bool {
    let dc: Vec<char> = d.chars().collect();
    r.windows(2).any(|w| {
        w[0].1 + 1 == w[1].0
            && dc[w[0].1] == ' '
            && first.contains(&w[0].2.as_str())
            && second.contains(&w[1].2.as_str())
    })
}

fn kind_of(d: &str, r: &[(usize, usize, String)]) -> &'static str {
    if has_any(
        r,
        &["fix", "bug", "repair", "finding", "findings", "regression"],
    ) {
        "fix"
    } else if has_any(r, &["refactor", "rename", "restructure", "cleanup"]) {
        "refactor"
    } else if has_any(
        r,
        &[
            "test", "tests", "e2e", "charter", "charters", "journey", "journeys", "load",
        ],
    ) || has_pair(d, r, &["perf"], &["suite", "suites"])
    {
        "test-only"
    } else if has_any(r, &["doc", "docs", "documentation", "readme", "discovery"]) {
        "docs"
    } else if has_any(r, &["ci", "pipeline", "release", "workflow", "dependabot"]) {
        "ci"
    } else if has_any(
        r,
        &[
            "foundation",
            "skeleton",
            "scaffold",
            "bootstrap",
            "setup",
            "new",
        ],
    ) {
        "new-module"
    } else {
        "extend"
    }
}

fn surface_of(r: &[(usize, usize, String)]) -> &'static str {
    if has_any(
        r,
        &[
            "protocol", "ipc", "socket", "wire", "openapi", "contract", "daemon",
        ],
    ) {
        "protocol"
    } else if has_any(
        r,
        &[
            "storage",
            "store",
            "database",
            "sqlite",
            "migration",
            "schema",
        ],
    ) {
        "storage"
    } else if has_any(r, &["snapshot", "snapshots"]) {
        "ui-snapshots"
    } else if has_any(
        r,
        &[
            "picker", "tui", "terminal", "display", "tray", "render", "window", "capture", "theme",
            "color", "colour",
        ],
    ) {
        "session-display"
    } else if has_any(r, &["ci", "pipeline", "release", "workflow"])
        || r.iter().any(|(_, _, w)| w.starts_with("orchestrat"))
    {
        "workflow"
    } else {
        "other"
    }
}

fn layer_word(r: &[(usize, usize, String)]) -> Option<&'static str> {
    if has_any(r, &["cli"]) {
        Some("cli")
    } else if has_any(r, &["tui"]) {
        Some("tui")
    } else if has_any(
        r,
        &[
            "container",
            "docker",
            "devcontainer",
            "compose",
            "terraform",
        ],
    ) {
        Some("infra")
    } else {
        None
    }
}

/// Coarse tags for work nobody tagged, from its description, the files it touched and the repo's
/// manifests. Low confidence by design; novelty and verification stay unknown.
#[must_use]
pub fn infer_tags(
    desc: &str,
    layer: Option<&str>,
    ntasks: usize,
    files: &[String],
    lines: i64,
    repo: Option<&RepoInfo>,
) -> Tags {
    let d = desc.to_lowercase();
    let r = runs(&d);
    let mut tags: Tags = vec![("kind".into(), J::s(kind_of(&d, &r)))];
    let layer_values = facet("layer");
    let lay = match layer {
        Some(l) if layer_values.is_some_and(|f| f.values.contains_key(l)) => Some(l),
        _ => layer_word(&r),
    };
    if let Some(l) = lay {
        tags.push(("layer".into(), J::s(l)));
    }
    tags.push(("surface".into(), J::s(surface_of(&r))));
    if let Some(lang) = agent_lang(files, repo) {
        tags.push(("lang".into(), J::s(lang.clone())));
        let st: Vec<J> = repo
            .map(|r| r.stack.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|v| {
                facet("stack")
                    .and_then(|f| f.values.get(v))
                    .and_then(J::as_str)
                    == Some(lang.as_str())
            })
            .map(J::s)
            .collect();
        if !st.is_empty() {
            tags.push(("stack".into(), J::Arr(st)));
        }
    }
    let nfiles = files.len();
    if nfiles > 0 || lines != 0 {
        let size = if nfiles <= 3 && lines < 150 {
            "S"
        } else if nfiles <= 10 && lines < 600 {
            "M"
        } else {
            "L"
        };
        tags.push(("size".into(), J::s(size)));
    } else if let Some(s) = size_of(Some(ntasks)) {
        tags.push(("size".into(), J::s(s)));
    }
    if layer == Some("qa") || r.iter().any(|(_, _, w)| w == "e2e") {
        tags.push(("verification".into(), J::s("e2e")));
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags_json(t: &Tags) -> String {
        pyjson::dumps(&J::Obj(t.clone()), false, None)
    }

    #[test]
    fn parse_format_and_aliases() {
        let t = parse_tags("M, tokio, rust, extend, react-dom, bogus, backend");
        let get = |f: &str| tag_get(&t, f).cloned();
        assert_eq!(get("size"), Some(J::s("M")));
        assert_eq!(
            get("stack"),
            Some(J::Arr(vec![J::s("react"), J::s("tokio")]))
        );
        assert_eq!(get("lang"), Some(J::s("rust")));
        assert_eq!(get("kind"), Some(J::s("extend")));
        assert_eq!(get("layer"), Some(J::s("backend")));
        assert_eq!(t.len(), 5);
        assert_eq!(format_tags(&t), "extend, backend, rust, react, tokio, M");
        assert_eq!(norm_stack("left-pad"), None);
        assert_eq!(norm_stack("serde_json").as_deref(), Some("serde"));
    }

    #[test]
    fn infer_tags_from_description_files_and_repo() {
        let repo = RepoInfo {
            lang: Some("rust".into()),
            langs: vec![],
            stack: vec!["tokio".into(), "react".into()],
        };
        let files = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let t = infer_tags(
            "Fix daemon charter findings",
            Some("backend"),
            0,
            &files,
            40,
            Some(&repo),
        );
        let get = |f: &str| tag_get(&t, f).cloned();
        assert_eq!(get("kind"), Some(J::s("fix")));
        assert_eq!(get("surface"), Some(J::s("protocol")));
        assert_eq!(get("lang"), Some(J::s("rust")));
        assert_eq!(get("stack"), Some(J::Arr(vec![J::s("tokio")])));
        assert_eq!(get("size"), Some(J::s("S")));
        assert_eq!(get("layer"), Some(J::s("backend")));
        assert_eq!(get("novelty"), None);
        let md = infer_tags(
            "Write the docs",
            None,
            0,
            &["a.md".to_string()],
            10,
            Some(&repo),
        );
        assert_eq!(tag_get(&md, "lang"), Some(&J::s("markdown")));
        assert_eq!(
            tags_json(&infer_tags("x", None, 0, &[], 0, None)),
            r#"{"kind": "extend", "surface": "other"}"#
        );
    }

    #[test]
    fn repo_info_reads_manifests_read_only() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("r");
        std::fs::create_dir_all(repo.join("web")).unwrap();
        std::fs::write(
            repo.join("Cargo.toml"),
            "[package]\nname=\"x\"\n[dependencies]\ntokio = \"1\"\nserde_json = \"1\"\nleft_pad = \"1\"\n[dev-dependencies.clap]\nversion=\"4\"\n",
        )
        .unwrap();
        std::fs::write(
            repo.join("web/package.json"),
            r#"{"dependencies": {"react-dom": "1"}, "devDependencies": {"typescript": "5", "vitest": "1"}}"#,
        )
        .unwrap();
        let info = repo_info(&repo);
        assert_eq!(info.lang.as_deref(), Some("rust"));
        assert_eq!(info.langs, ["rust", "typescript"]);
        assert_eq!(info.stack, ["clap", "react", "serde", "tokio", "vitest"]);
        assert_eq!(repo_info(&d.path().join("none")).lang, None);
    }

    #[test]
    fn go_and_python_manifests() {
        assert_eq!(
            go_names("module x\nrequire github.com/a/b v1.0.0\n\tgolang.org/x/net v0.1.0\n"),
            ["github.com/a/b", "golang.org/x/net"]
        );
        assert_eq!(
            pyproject_names("dependencies = [\"fastapi>=0.1\", 'uvicorn[standard]', \"x\"]"),
            ["fastapi", "uvicorn", "x"]
        );
    }

    #[test]
    fn embedded_vocabulary_equals_the_shared_file() {
        let shared = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../shared/token-tags.json"
        ))
        .unwrap();
        assert_eq!(
            shared, VOCAB_JSON,
            "run: python3 scripts/consistency.py --sync-shared-code"
        );
    }
}
