use compactcore::{compact, Lang};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = std::fs::read_to_string(&a[1]).unwrap();
    let ext = a[1].rsplit('.').next().unwrap();
    let (out, info) = compact(&src, Lang::from_ext(ext).unwrap(), a[2].parse().unwrap()).unwrap();
    eprintln!("{info:?}");
    print!("{out}");
}
