//! Crate-level doc comment.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fmt;

/// A key-value cache with a lifetime-bound view.
#[derive(Debug, Default, Clone)]
pub struct Cache {
    map: HashMap<String, Vec<i32>>,
}

pub struct View<'a> {
    name: &'a str,
}

impl<'a> fmt::Display for View<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "View({})", self.name)
    }
}

impl Cache {
    pub fn insert(&mut self, k: &str, v: i32) -> &mut Self {
        self.map.entry(k.to_string()).or_insert_with(Vec::new).push(v);
        self
    }

    pub fn sum<T>(&self, key: T) -> Option<i32>
    where
        T: AsRef<str>,
    {
        self.map.get(key.as_ref()).map(|v| v.iter().sum())
    }
}

fn classify(n: i64) -> &'static str {
    match n {
        i64::MIN..=-1 => "neg",
        0 => "zero",
        1..=9 => "small",
        _ => "big",
    }
}

fn main() {
    let mut c = Cache::default();
    c.insert("a", 1).insert("a", 2).insert("b", -3);
    let raw = r#"raw "string"
    keeps   spacing"#;
    let ch = 'x';
    let v: Vec<Vec<u8>> = vec![vec![1, 2], vec![3]];
    let total: usize = v.iter().map(|x| x.len()).sum::<usize>();
    let r = &&5;
    let neg = 3 - -2;
    let range: Vec<_> = (0..3).collect();
    let view = View { name: "main" };
    let closure = |a: i32, b: i32| -> i32 { a * b };
    println!("{} {:?} {} {} {} {} {:?} {} {}", raw, c.sum("a"), ch, total, **r, neg, range, view, closure(6, 7));
    println!("{} {} {}", classify(-5), classify(0), classify(42));
}
