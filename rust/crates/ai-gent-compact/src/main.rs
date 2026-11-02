//! `ai-gent-compact`: read source code token-lean (a compact view and an outline). Read-only: the
//! only file it writes is the one named by `-o`.

#![allow(
    clippy::struct_excessive_bools,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::format_push_string
)]

use ai_gent_compact::{Options, compact_text, langs, walk};
use clap::Parser;
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;

/// Strip non-semantic formatting from source code (a token-lean view).
#[derive(Parser)]
#[command(name = "ai-gent-compact", version = ai_gent_compact::VERSION)]
struct Cli {
    /// Files, directories, or - for stdin
    paths: Vec<String>,
    /// Force the language (required for stdin)
    #[arg(short, long)]
    lang: Option<String>,
    /// Level 0 (as is) to 4 (dense)
    #[arg(short = 'L', long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(0..=4))]
    level: u8,
    /// Keep one blank line where the source had blank lines
    #[arg(short, long)]
    paragraphs: bool,
    /// Also drop comments (directive comments like //go:build are kept)
    #[arg(short = 'c', long)]
    strip_comments: bool,
    /// Keep line breaks at brace depth <= DEPTH (0 = one top-level item per line)
    #[arg(long, value_name = "DEPTH")]
    outline: Option<usize>,
    /// Prefix each output line with its original line number
    #[arg(short, long)]
    numbers: bool,
    /// Soft-wrap long lines at an original line break (0 = never)
    #[arg(short, long, default_value_t = 1500)]
    wrap: usize,
    /// Write the combined output to this file
    #[arg(short, long)]
    output: Option<String>,
    /// No per-file summary on stderr
    #[arg(short, long)]
    quiet: bool,
    /// Skip the safety check
    #[arg(long)]
    no_verify: bool,
    /// Accepted for compatibility: the grammar check is part of the check
    #[arg(long)]
    no_ast: bool,
    /// List the supported languages
    #[arg(long)]
    list_languages: bool,
}

fn main() -> ExitCode {
    let a = Cli::parse();
    if a.list_languages {
        print!("{}", langs::list_languages());
        return ExitCode::SUCCESS;
    }
    if a.paths.is_empty() {
        eprintln!("usage: ai-gent-compact [OPTIONS] PATH... (see --help)");
        return ExitCode::from(1);
    }
    let mut files: Vec<String> = Vec::new();
    for p in &a.paths {
        if p != "-" && Path::new(p).is_dir() {
            files.extend(
                walk::source_files(Path::new(p))
                    .iter()
                    .map(|f| f.to_string_lossy().into_owned()),
            );
        } else {
            files.push(p.clone());
        }
    }
    let multi = files.len() > 1;
    let opts = Options {
        level: a.level,
        strip_comments: a.strip_comments,
        outline: a.outline,
        wrap: a.wrap,
        numbers: a.numbers,
        check: !a.no_verify,
        paragraphs: a.paragraphs,
    };
    let (mut chunks, mut rc) = (Vec::new(), 0u8);
    for path in &files {
        let lang = match &a.lang {
            Some(l) => match langs::resolve(l) {
                Some(k) => Some(k),
                None => {
                    eprintln!("ai-gent-compact: unknown language {l:?} (see --list-languages)");
                    return ExitCode::from(1);
                }
            },
            None if path == "-" => None,
            None => langs::detect(path),
        };
        let mut bytes = Vec::new();
        let read = if path == "-" {
            std::io::stdin().read_to_end(&mut bytes).map(|_| ())
        } else {
            std::fs::read(path).map(|b| bytes = b)
        };
        if let Err(e) = read {
            eprintln!("ai-gent-compact: {path}: {e}");
            rc = 1;
            continue;
        }
        if bytes.contains(&0) {
            eprintln!("ai-gent-compact: skipping binary file {path}");
            continue;
        }
        let code = String::from_utf8_lossy(&bytes).into_owned();
        let (text, info) = match lang {
            Some(k) => compact_text(&code, k, &opts),
            None => {
                if path == "-" || !multi {
                    eprintln!("ai-gent-compact: unknown language for {path} (use -l)");
                }
                (code.clone(), ai_gent_compact::Info::unknown())
            }
        };
        if !a.quiet {
            let (before, after) = (code.chars().count(), text.chars().count());
            let pct = if before == 0 {
                0.0
            } else {
                100.0 * (after as f64 - before as f64) / before as f64
            };
            let mut extra = String::new();
            if !info.fallback.is_empty() {
                extra += &format!(" fallback[{}]", info.fallback.join(","));
            }
            if let Some(e) = &info.error {
                extra += &format!(" kept-as-is({e})");
            }
            if let Some(n) = &info.note {
                extra += &format!(" {n}");
            }
            eprintln!(
                "compact: {path} {} L{} {before}->{after} chars ({pct:+.1}%) verify={}{extra}",
                info.lang,
                info.level(),
                info.verified
            );
        }
        chunks.push(if multi {
            format!("==> {path} <==\n{text}")
        } else {
            text
        });
    }
    let result = chunks.join("\n");
    let wrote = match &a.output {
        Some(f) => std::fs::write(f, result.as_bytes()),
        None => std::io::stdout()
            .write_all(result.as_bytes())
            .and_then(|()| std::io::stdout().flush()),
    };
    if let Err(e) = wrote {
        eprintln!("ai-gent-compact: {e}");
        return ExitCode::from(1);
    }
    ExitCode::from(rc)
}
