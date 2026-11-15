//! Column and free-text masking: what a result may show unless the user asked to reveal it.
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

macro_rules! re {
    ($p:expr) => {{
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new($p).unwrap())
    }};
}

fn rules() -> &'static Vec<(&'static str, Regex)> {
    static R: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    R.get_or_init(|| {
        [
            ("secret", r"(?i)pass(word|wd)?$|^pwd$|senha|secret|token|api_?key|hash|salt|private_?key|credential|access_?key"),
            ("email", r"(?i)e_?mail|^mail$|_mail$"),
            ("document", r"(?i)(^|_)(cpf|cnpj|rg|ssn|nif|nie|cns|pis|nis|passport|passaporte|documento?|tax_?id|renavam|cnh)($|_)"),
            ("phone", r"(?i)phone|telefone|fone|celular|mobile|whatsapp|(^|_)tel($|_)"),
            ("card", r"(?i)(^|_)(card|cartao|pan|cc_?number)($|_)"),
            ("birth", r"(?i)birth|nascimento|(^|_)dob($|_)|data_nasc"),
            ("address", r"(?i)address|endereco|logradouro|street|(^|_)rua($|_)|(^|_)(cep|zip|postal)(_code)?($|_)|complemento|bairro"),
            ("ip", r"(?i)(^|_)ip(_?addr(ess)?)?($|_)|remote_addr"),
            ("name", r"(?i)^(nome|name|first_?name|last_?name|full_?name|surname|sobrenome|display_?name|nome_\w+|\w+_nome|mae|pai|mother|father|nome_social)$"),
        ]
        .iter()
        .map(|(k, p)| (*k, Regex::new(p).unwrap()))
        .collect()
    })
}

fn email_sub(s: &str) -> String {
    re!(r"([A-Za-z0-9._%+-])[A-Za-z0-9._%+-]*@([A-Za-z0-9])[A-Za-z0-9.-]*(\.[A-Za-z]{2,})")
        .replace_all(s, "${1}***@${2}***${3}")
        .into_owned()
}

/// The Luhn checksum of a digit string.
#[must_use]
pub fn luhn(digits: &str) -> bool {
    let (mut total, mut alt) = (0u32, false);
    for ch in digits.chars().rev() {
        let mut n = ch.to_digit(10).unwrap_or(0);
        if alt {
            n = if n > 4 { n * 2 - 9 } else { n * 2 };
        }
        total += n;
        alt = !alt;
    }
    total.is_multiple_of(10)
}

/// Masks every digit of `v` except the last `keep`.
#[must_use]
pub fn keep_last(v: &str, keep: usize) -> String {
    let digits = v.chars().filter(char::is_ascii_digit).count();
    let mut seen = 0;
    v.chars()
        .map(|ch| {
            if ch.is_ascii_digit() {
                seen += 1;
                if seen + keep > digits { ch } else { '*' }
            } else {
                ch
            }
        })
        .collect()
}

/// Masks one value for a column kind (`secret`, `email`, `document`, `phone`, `card`, `birth`,
/// `address`, `ip`, `name`).
#[must_use]
pub fn mask_value(kind: &str, v: &Value) -> Value {
    if v.is_null() {
        return Value::Null;
    }
    let s = match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    Value::String(match kind {
        "secret" => "[redacted]".into(),
        "email" => {
            if s.contains('@') {
                email_sub(&s)
            } else {
                "***".into()
            }
        }
        "document" | "phone" => keep_last(&s, 2),
        "card" => keep_last(&s, 4),
        "birth" => match re!(r"^(\d{4})").captures(&s) {
            Some(m) => format!("{}-**-**", &m[1]),
            None => "****".into(),
        },
        "address" => "[masked]".into(),
        "ip" => {
            if s.contains('.') {
                re!(r"(\d+\.\d+)\.\d+\.\d+")
                    .replace_all(&s, "${1}.*.*")
                    .into_owned()
            } else {
                "[masked]".into()
            }
        }
        "name" => {
            let o: Vec<String> = s
                .split_whitespace()
                .map(|w| format!("{}***", w.chars().next().unwrap()))
                .collect();
            if o.is_empty() {
                "***".into()
            } else {
                o.join(" ")
            }
        }
        _ => s,
    })
}

/// Masks emails, CPF, CNPJ and Luhn-valid card numbers inside free text.
#[must_use]
pub fn mask_free_text(s: &str) -> String {
    let s = email_sub(s);
    let s = re!(r"\b\d{3}\.\d{3}\.\d{3}-\d{2}\b")
        .replace_all(&s, |m: &regex::Captures| keep_last(&m[0], 2))
        .into_owned();
    let s = re!(r"\b\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}\b")
        .replace_all(&s, |m: &regex::Captures| keep_last(&m[0], 2))
        .into_owned();
    re!(r"\b(?:\d[ -]?){13,19}\b")
        .replace_all(&s, |m: &regex::Captures| {
            let d: String = m[0].chars().filter(char::is_ascii_digit).collect();
            if (13..=19).contains(&d.len()) && luhn(&d) {
                keep_last(&m[0], 4)
            } else {
                m[0].to_string()
            }
        })
        .into_owned()
}

/// Returns (masked rows, column -> kind). With `reveal`, only `secret` columns stay masked.
#[must_use]
pub fn mask_result(
    columns: &[String],
    rows: &[Vec<Value>],
    reveal: bool,
) -> (Vec<Vec<Value>>, Vec<(String, String)>) {
    let mut kinds: Vec<Option<&str>> = vec![None; columns.len()];
    for (i, col) in columns.iter().enumerate() {
        let base = col
            .rsplit('.')
            .next()
            .unwrap()
            .trim_matches(|c| "\"`[]".contains(c))
            .to_lowercase();
        for (k, p) in rules() {
            if p.is_match(&base) {
                kinds[i] = Some(k);
                break;
            }
        }
        if reveal && kinds[i] != Some("secret") {
            kinds[i] = None;
        }
    }
    let masked = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, v)| match (kinds.get(i).copied().flatten(), v) {
                    (Some(k), v) => mask_value(k, v),
                    (None, _) if reveal => v.clone(),
                    (None, Value::String(s)) => Value::String(mask_free_text(s)),
                    (None, v) => v.clone(),
                })
                .collect()
        })
        .collect();
    let km = kinds
        .iter()
        .enumerate()
        .filter_map(|(i, k)| k.map(|k| (columns[i].clone(), k.to_string())))
        .collect();
    (masked, km)
}
