use std::cmp::Ordering;

use best::{ResultData, ResultSet, ResultType, ResultValue};

/// Formats result sets exactly like EdiabasTest (Program.cs) prints them, starting at the first "DATASET:" line.
///
/// Mirrors `PrintResults` with default options (no `--alltypes`, no `-f` format, no `--hidetelrespbyte`):
/// - `DATASET: n` header per set (n starts at 0).
/// - Keys sorted with culture "en" collation (`OrderBy(x => x)`), see `collate`.
/// - Line: `NAME: <value>`, values as described in `format_value`.
/// - One empty line after all sets of one call (also when the list is empty).
pub fn format_result_sets(sets: &[ResultSet]) -> String {
    let mut out = String::new();
    for (index, set) in sets.iter().enumerate() {
        out.push_str(&format!("DATASET: {}\n", index));
        let mut items: Vec<&ResultData> = set.0.iter().collect();
        items.sort_by(|a, b| collate(&a.name, &b.name));
        for item in items {
            out.push_str(&item.name);
            out.push_str(": ");
            out.push_str(&format_value(item));
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

/// Value part of one result line (C# `sbResult`, default options).
fn format_value(data: &ResultData) -> String {
    match &data.value {
        // C# string branch: raw text, type ignored.
        ResultValue::Str(s) => s.clone(),
        // C# double branch: "R:" regardless of ResType, always 6 decimals.
        ResultValue::Float(f) => format!("R: {}", format_float(*f)),
        ResultValue::Int(v) => {
            let v = *v;
            match data.ty {
                ResultType::B => format!("B: {} 0x{:02X}", v, v as u8),
                ResultType::C => format!("C: {} 0x{:02X}", v, v as u8),
                ResultType::W => format!("W: {} 0x{:04X}", v, v as u16),
                ResultType::I => format!("I: {} 0x{:04X}", v, v as u16),
                ResultType::D => format!("D: {} 0x{:08X}", v, v as u32),
                ResultType::L => format!("L: {} 0x{:08X}", v, v as u32),
                // Oracle quirk: Q prints "QW:".
                ResultType::Q => format!("QW: {} 0x{:016X}", v, v as u64),
                // Oracle quirk: LL prints "L:" with 64-bit hex.
                ResultType::LL => format!("L: {} 0x{:016X}", v, v as u64),
                // C# default branch: integer stored with a non-integer type.
                ResultType::R | ResultType::S | ResultType::Y => "?".to_string(),
            }
        }
        // C# byte[] branch: each byte as "XX " (trailing space, also after the last byte).
        ResultValue::Bytes(bytes) => bytes.iter().map(|b| format!("{:02X} ", b)).collect(),
    }
}

/// C# `{0:0.000000}` with culture "en".
fn format_float(f: f64) -> String {
    if f.is_nan() {
        "NaN".to_string()
    } else if f == f64::INFINITY {
        "Infinity".to_string()
    } else if f == f64::NEG_INFINITY {
        "-Infinity".to_string()
    } else {
        format!("{:.6}", f)
    }
}

/// Approximation of .NET culture-"en" string comparison (ICU): punctuation < digits < letters,
/// letters compared case-insensitively first, then lowercase before uppercase as tie-break.
fn collate(a: &str, b: &str) -> Ordering {
    fn primary(c: char) -> (u8, char) {
        if c.is_ascii_alphabetic() {
            (2, c.to_ascii_lowercase())
        } else if c.is_ascii_digit() {
            (1, c)
        } else {
            (0, c)
        }
    }
    let mut ai = a.chars();
    let mut bi = b.chars();
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => break,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = primary(x).cmp(&primary(y));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
    // Tertiary: first case difference, lowercase sorts first.
    for (x, y) in a.chars().zip(b.chars()) {
        if x != y {
            return match (x.is_lowercase(), y.is_lowercase()) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => x.cmp(&y),
            };
        }
    }
    Ordering::Equal
}
