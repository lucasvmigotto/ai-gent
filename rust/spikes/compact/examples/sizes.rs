//! `cargo run --release --example sizes DIR`: bytes and reached level of every in-scope fixture at
//! levels 1-4 (the Rust half of the SC-005 comparison against `compact.py`).
use compactcore::{compact, Lang};
fn main() {
    let dir = std::env::args().nth(1).expect("fixtures dir");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    for name in names {
        let Some(lang) = Lang::from_ext(name.rsplit('.').next().unwrap()) else {
            continue;
        };
        let src = std::fs::read_to_string(format!("{dir}/{name}")).unwrap();
        let mut row = format!("{name} {}", src.len());
        for level in 1..=4 {
            match compact(&src, lang, level) {
                Ok((out, info)) => {
                    row += &format!(
                        " L{level}={}@{}{:?}",
                        out.len(),
                        info.level(),
                        info.fallback
                    )
                }
                Err(e) => row += &format!(" L{level}=ERR({e})"),
            }
        }
        println!("{row}");
    }
}
