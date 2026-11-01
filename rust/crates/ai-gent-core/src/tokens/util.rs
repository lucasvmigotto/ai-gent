//! Python-compatible helpers: ordered maps, rounding, number formatting, UTC time, text scanning.
use std::collections::HashMap;
use std::path::Path;

/// An insertion-ordered map where re-setting a key keeps its position (a Python `dict`).
#[derive(Debug, Clone)]
pub struct OMap<V> {
    items: Vec<(String, V)>,
    idx: HashMap<String, usize>,
}

impl<V> Default for OMap<V> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            idx: HashMap::new(),
        }
    }
}

impl<V> OMap<V> {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set or replace in place.
    pub fn set(&mut self, k: &str, v: V) {
        if let Some(&i) = self.idx.get(k) {
            self.items[i].1 = v;
        } else {
            self.idx.insert(k.to_string(), self.items.len());
            self.items.push((k.to_string(), v));
        }
    }

    /// Lookup.
    #[must_use]
    pub fn get(&self, k: &str) -> Option<&V> {
        self.idx.get(k).map(|&i| &self.items[i].1)
    }

    /// Mutable lookup.
    pub fn get_mut(&mut self, k: &str) -> Option<&mut V> {
        match self.idx.get(k) {
            Some(&i) => Some(&mut self.items[i].1),
            None => None,
        }
    }

    /// The value for `k`, creating it with `make()` first when absent (`defaultdict`).
    pub fn entry(&mut self, k: &str, make: impl FnOnce() -> V) -> &mut V {
        if !self.idx.contains_key(k) {
            self.set(k, make());
        }
        let i = self.idx[k];
        &mut self.items[i].1
    }

    /// Membership.
    #[must_use]
    pub fn contains_key(&self, k: &str) -> bool {
        self.idx.contains_key(k)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// No entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.items.iter().map(|(k, v)| (k, v))
    }

    /// Values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.items.iter().map(|(_, v)| v)
    }

    /// Keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.items.iter().map(|(k, _)| k)
    }
}

/// Python 3.12+ `sum()` of floats (Neumaier compensated).
#[must_use]
pub fn pysum(xs: &[f64]) -> f64 {
    let mut it = xs.iter();
    let Some(&first) = it.next() else { return 0.0 };
    let (mut s, mut c) = (first, 0.0f64);
    for &x in it {
        let t = s + x;
        if s.abs() >= x.abs() {
            c += (s - t) + x;
        } else {
            c += (x - t) + s;
        }
        s = t;
    }
    if c != 0.0 && c.is_finite() { s + c } else { s }
}

/// Python `round(x)` to an integer (half to even).
#[must_use]
pub fn round_i(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// Python `round(x, 2)`.
#[must_use]
pub fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// Python's `\w` for one character.
#[must_use]
pub fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Python `str.splitlines()` for `\n`, `\r\n` and `\r`.
#[must_use]
pub fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let (mut start, mut i) = (0, 0);
    while i < b.len() {
        match b[i] {
            b'\n' => {
                out.push(&s[start..i]);
                i += 1;
                start = i;
            }
            b'\r' => {
                out.push(&s[start..i]);
                i += if b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < b.len() {
        out.push(&s[start..]);
    }
    out
}

/// Read a text file like Python's `open(path, encoding="utf-8")`: strict UTF-8, universal newlines.
pub fn read_text(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let s = String::from_utf8(bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s
    })
}

/// Create the parent directory of a path (`os.makedirs(dirname(abspath(path)), exist_ok=True)`).
pub fn ensure_parent(path: &Path) -> std::io::Result<()> {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => std::fs::create_dir_all(p),
        _ => Ok(()),
    }
}

/// Write a file through a temp file in the same directory and rename it over the target, so a
/// reader (or a crash) never sees half a file.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    ensure_parent(path)?;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!(".{name}.tmp{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// `os.path.expanduser` for a leading `~`.
#[must_use]
pub fn expanduser(p: &str) -> String {
    if p == "~" || p.starts_with("~/") || p.starts_with("~\\") {
        if let Some(h) = home() {
            return format!("{h}{}", &p[1..]);
        }
    }
    p.to_string()
}

/// The home directory (`$HOME`, else `%USERPROFILE%`).
#[must_use]
pub fn home() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("USERPROFILE").ok().filter(|v| !v.is_empty()))
}

/// Entries of a directory sorted by path, hidden ones (leading dot) skipped, like `glob("*")`.
#[must_use]
pub fn glob_children(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<_> = rd
        .filter_map(Result::ok)
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

// ------------------------------------------------------------ number formats

fn commas(s: &str) -> String {
    let (int, rest) = s.find('.').map_or((s, ""), |i| (&s[..i], &s[i..]));
    let (neg, digits) = int.strip_prefix('-').map_or(("", int), |d| ("-", d));
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{neg}{out}{rest}")
}

/// `k()`: `391.4k` or `1.24M` (with thousands commas).
#[must_use]
pub fn k(n: i64) -> String {
    if n < 1_000_000 {
        format!("{}k", commas(&format!("{:.1}", n as f64 / 1000.0)))
    } else {
        format!("{}M", commas(&format!("{:.2}", n as f64 / 1_000_000.0)))
    }
}

/// `fmt_tok()`: `391k`, `1.24M`, an em dash for none.
#[must_use]
pub fn fmt_tok(n: Option<i64>) -> String {
    match n {
        None => "—".into(),
        Some(n) if n < 1_000_000 => format!("{}k", round_i(n as f64 / 1000.0)),
        Some(n) => format!("{:.2}M", n as f64 / 1_000_000.0),
    }
}

/// `parse_tok()`: `~420k` is 420000, `1.2M` is 1200000, `391,000` is 391000, `—` is none.
#[must_use]
pub fn parse_tok(s: &str) -> Option<i64> {
    let t = s.trim_matches(|c: char| c.is_whitespace() || "`*~ ".contains(c));
    let t: String = t.chars().filter(|&c| c != ',' && c != '_').collect();
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == i + 1 {
            return None;
        }
        i = j;
    }
    let num = &t[..i];
    let mult = match t[i..].trim_start() {
        "" => 1.0,
        "k" | "K" => 1_000.0,
        "m" | "M" => 1_000_000.0,
        _ => return None,
    };
    Some(round_i(num.parse::<f64>().ok()? * mult))
}

/// `pct_error()`: `+12%`, `-3%` or an em dash.
#[must_use]
pub fn pct_error(projected: Option<i64>, used: Option<i64>) -> String {
    match (projected, used) {
        (Some(p), Some(u)) if p != 0 => format!("{:+.0}%", (u - p) as f64 / p as f64 * 100.0),
        _ => "—".into(),
    }
}

/// `format(x, 'g')` for the small values the budget prints.
#[must_use]
pub fn fmt_g(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    let exp = x.abs().log10().floor() as i32;
    if !(-4..6).contains(&exp) {
        let s = format!("{x:.5e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') {
            m.trim_end_matches('0').trim_end_matches('.')
        } else {
            m
        };
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs());
    }
    let decimals = (5 - exp).max(0) as usize;
    let s = format!("{x:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// `s[:n]` by characters.
#[must_use]
pub fn head(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ------------------------------------------------------------------- time

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn num(b: &[u8], i: &mut usize, n: usize) -> Option<i64> {
    let s = b.get(*i..*i + n)?;
    if !s.iter().all(u8::is_ascii_digit) {
        return None;
    }
    *i += n;
    std::str::from_utf8(s).ok()?.parse().ok()
}

/// `datetime.fromisoformat(ts.replace("Z", "+00:00"))` as seconds since the epoch (UTC; a naive
/// time is read as UTC). `None` for text Python would reject.
#[must_use]
pub fn parse_iso(ts: &str) -> Option<i64> {
    let t = ts.replace('Z', "+00:00");
    let b = t.as_bytes();
    let mut i = 0;
    let y = num(b, &mut i, 4)?;
    if b.get(i) != Some(&b'-') {
        return None;
    }
    i += 1;
    let mo = num(b, &mut i, 2)?;
    if b.get(i) != Some(&b'-') {
        return None;
    }
    i += 1;
    let d = num(b, &mut i, 2)?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut h, mut mi, mut s) = (0, 0, 0);
    let mut off = 0;
    if i < b.len() {
        i += 1; // the separator ('T' or ' ')
        h = num(b, &mut i, 2)?;
        if b.get(i) == Some(&b':') {
            i += 1;
            mi = num(b, &mut i, 2)?;
            if b.get(i) == Some(&b':') {
                i += 1;
                s = num(b, &mut i, 2)?;
                if matches!(b.get(i), Some(b'.' | b',')) {
                    i += 1;
                    let st = i;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i == st {
                        return None;
                    }
                }
            }
        }
        if i < b.len() {
            let sign = match b[i] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            i += 1;
            let oh = num(b, &mut i, 2)?;
            if b.get(i) == Some(&b':') {
                i += 1;
            }
            let om = num(b, &mut i, 2).unwrap_or(0);
            off = sign * (oh * 3600 + om * 60);
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b':') {
                i += 1;
            }
        }
        if i < b.len() || h > 24 || mi > 59 || s > 59 {
            return None;
        }
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - off)
}

/// `YYYY-MM-DDTHH:MM:SS` of an epoch.
#[must_use]
pub fn fmt_epoch(e: i64) -> String {
    let days = e.div_euclid(86_400);
    let rem = e.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// `to_utc()`: an ISO timestamp as `YYYY-MM-DDTHH:MM:SS` in UTC.
#[must_use]
pub fn to_utc(ts: &str) -> Option<String> {
    parse_iso(ts).map(fmt_epoch)
}

/// Seconds since the epoch now.
#[must_use]
pub fn system_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// --------------------------------------------------------- text scanning

/// Spans (in chars) of `\bT\d{3,}\b`.
#[must_use]
pub fn task_id_spans(c: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == 'T' && (i == 0 || !is_word(c[i - 1])) {
            let mut j = i + 1;
            while j < c.len() && c[j].is_ascii_digit() {
                j += 1;
            }
            if j - i > 3 && (j >= c.len() || !is_word(c[j])) {
                out.push((i, j));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `TASK_ID.findall(text)`.
#[must_use]
pub fn task_ids(text: &str) -> Vec<String> {
    let c: Vec<char> = text.chars().collect();
    task_id_spans(&c)
        .into_iter()
        .map(|(a, b)| c[a..b].iter().collect())
        .collect()
}

fn digits_end(c: &[char], i: usize) -> usize {
    let mut j = i;
    while j < c.len() && c[j].is_ascii_digit() {
        j += 1;
    }
    j
}

/// One `\bT(\d{3,})\s*(?:–|—|-|\.\.|\bto\b)\s*T?(\d{3,})\b` match starting exactly at `i`.
fn range_at(c: &[char], i: usize) -> Option<(usize, String, String)> {
    if c[i] != 'T' || (i > 0 && is_word(c[i - 1])) {
        return None;
    }
    let j = digits_end(c, i + 1);
    if j - i - 1 < 3 {
        return None;
    }
    let lo: String = c[i + 1..j].iter().collect();
    let mut k = j;
    while k < c.len() && c[k].is_whitespace() {
        k += 1;
    }
    let after = match c.get(k)? {
        '–' | '—' | '-' => k + 1,
        '.' if c.get(k + 1) == Some(&'.') => k + 2,
        't' if c.get(k + 1) == Some(&'o')
            && (k == 0 || !is_word(c[k - 1]))
            && c.get(k + 2).is_none_or(|&x| !is_word(x)) =>
        {
            k + 2
        }
        _ => return None,
    };
    let mut m = after;
    while m < c.len() && c[m].is_whitespace() {
        m += 1;
    }
    if c.get(m) == Some(&'T') {
        m += 1;
    }
    let e = digits_end(c, m);
    if e - m < 3 || c.get(e).is_some_and(|&x| is_word(x)) {
        return None;
    }
    Some((e, lo, c[m..e].iter().collect()))
}

/// `expand_ids()`: task ids named in text, ranges expanded, in order, without duplicates.
#[must_use]
pub fn expand_ids(text: &str) -> Vec<String> {
    let c: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if let Some((end, lo_s, hi_s)) = range_at(&c, i) {
            if let (Ok(lo), Ok(hi)) = (lo_s.parse::<i64>(), hi_s.parse::<i64>()) {
                if lo <= hi && hi <= lo + 200 {
                    let width = lo_s.len();
                    out.extend((lo..=hi).map(|n| format!("T{n:0width$}")));
                    spans.push((i, end));
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    for (a, b) in task_id_spans(&c) {
        if !spans.iter().any(|&(s, e)| s <= a && a < e) {
            out.push(c[a..b].iter().collect());
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.into_iter().filter(|t| seen.insert(t.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_ids_and_tokens_format() {
        assert_eq!(
            expand_ids("T001–T003, T007 and T001"),
            ["T001", "T002", "T003", "T007"]
        );
        assert_eq!(expand_ids("T001 to T002"), ["T001", "T002"]);
        assert_eq!(expand_ids("T007, T001-T002"), ["T001", "T002", "T007"]);
        assert_eq!(expand_ids("T0010 T12 xT001 T001a"), ["T0010"]);
        assert_eq!(parse_tok("~420k"), Some(420_000));
        assert_eq!(parse_tok("1.5M"), Some(1_500_000));
        assert_eq!(parse_tok("391,000"), Some(391_000));
        assert_eq!(parse_tok("`400k`"), Some(400_000));
        assert_eq!(parse_tok("—"), None);
        assert_eq!(parse_tok(""), None);
        assert_eq!(fmt_tok(Some(391_400)), "391k");
        assert_eq!(fmt_tok(Some(1_240_000)), "1.24M");
        assert_eq!(fmt_tok(None), "—");
        assert_eq!(fmt_tok(Some(1500)), "2k");
        assert_eq!(fmt_tok(Some(2500)), "2k");
    }

    #[test]
    fn k_formats() {
        assert_eq!(k(2000), "2.0k");
        assert_eq!(k(1_125_000), "1.12M");
        assert_eq!(k(1_234_567_000), "1,234.57M");
    }

    #[test]
    fn time_roundtrip() {
        assert_eq!(
            to_utc("2026-10-20T10:00:00Z").unwrap(),
            "2026-10-20T10:00:00"
        );
        assert_eq!(
            to_utc("2026-10-19T02:00:00+02:00").unwrap(),
            "2026-10-19T00:00:00"
        );
        assert_eq!(
            to_utc("2026-10-19T00:00:00.123456+00:00").unwrap(),
            "2026-10-19T00:00:00"
        );
        assert_eq!(to_utc("2026-10-19").unwrap(), "2026-10-19T00:00:00");
        assert!(to_utc("nope").is_none());
        assert_eq!(fmt_epoch(0), "1970-01-01T00:00:00");
        assert_eq!(
            parse_iso("2024-02-29T23:59:59Z").map(fmt_epoch).unwrap(),
            "2024-02-29T23:59:59"
        );
    }

    #[test]
    fn g_format() {
        assert_eq!(fmt_g(35.0), "35");
        assert_eq!(fmt_g(0.5), "0.5");
        assert_eq!(fmt_g(1_000_000.0), "1e+06");
    }

    #[test]
    fn omap_keeps_position() {
        let mut m = OMap::new();
        m.set("a", 1);
        m.set("b", 2);
        m.set("a", 3);
        assert_eq!(
            m.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(),
            [("a", 3), ("b", 2)]
        );
    }

    /// The rows of an exported `[...]` table in `pct-fixtures.ts` (the same file the TypeScript
    /// toast tests and the Python tests read).
    fn ts_rows(name: &str, width: usize) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../plugins/agents/hooks/pct-fixtures.ts"
        ))
        .unwrap();
        let body = text
            .split(&format!("export const {name}"))
            .nth(1)
            .unwrap()
            .split("\n]\n")
            .next()
            .unwrap();
        let body = body.split("= [").nth(1).unwrap();
        let mut rows = Vec::new();
        let mut rest = body;
        while let Some(a) = rest.find('[') {
            let b = rest[a..].find(']').unwrap() + a;
            let cells: Vec<String> = rest[a + 1..b]
                .split(',')
                .map(|c| c.trim().trim_matches('\'').to_string())
                .collect();
            assert_eq!(cells.len(), width);
            rows.push(cells);
            rest = &rest[b + 1..];
        }
        assert!(!rows.is_empty());
        rows
    }

    #[test]
    fn pct_error_gives_each_fixture_percentage() {
        for r in ts_rows("PCT_FIXTURES", 3) {
            let (p, u): (i64, i64) = (r[0].parse().unwrap(), r[1].parse().unwrap());
            assert_eq!(pct_error(Some(p), Some(u)), r[2], "{p}/{u}");
        }
        assert_eq!(pct_error(None, Some(5)), "—");
        assert_eq!(pct_error(Some(0), Some(5)), "—");
        assert_eq!(pct_error(Some(5), None), "—");
    }

    #[test]
    fn cells_and_exact_figures_may_differ_by_a_point() {
        for r in ts_rows("PCT_GAP_FIXTURES", 6) {
            let n = |i: usize| r[i].parse::<i64>().unwrap();
            assert_eq!(pct_error(Some(n(0)), Some(n(1))), r[2]);
            assert_eq!(pct_error(Some(n(3)), Some(n(4))), r[5]);
            assert_eq!(
                (fmt_tok(Some(n(3))), fmt_tok(Some(n(4)))),
                (fmt_tok(Some(n(0))), fmt_tok(Some(n(1))))
            );
            let pct = |s: &str| s.trim_end_matches('%').parse::<i64>().unwrap();
            assert_eq!((pct(&r[2]) - pct(&r[5])).abs(), 1);
        }
    }
}
