//! Searching: hex patterns with wildcards, text in several encodings, byte
//! regexes and numbers. Everything compiles down to a `regex::bytes::Regex`
//! that is run over the document in overlapping chunks.

use crate::buffer::PieceTable;
use crate::inspector::{encode_value, NumType};
use crate::{Error, Task};
use regex::bytes::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

const CHUNK: u64 = 8 << 20;
/// Longest match a free-form regex is guaranteed to find across chunk borders.
const REGEX_OVERLAP: u64 = 64 << 10;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextEncoding {
    Utf8,
    Utf16le,
    Utf16be,
    Latin1,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Query {
    Hex {
        pattern: String,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        text: String,
        encoding: TextEncoding,
        case_sensitive: bool,
    },
    #[serde(rename_all = "camelCase")]
    Regex {
        pattern: String,
        case_sensitive: bool,
    },
    #[serde(rename_all = "camelCase")]
    Number {
        value: String,
        num_type: NumType,
        little_endian: bool,
    },
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct Match {
    pub offset: u64,
    pub len: u64,
}

pub struct Matcher {
    re: Regex,
    overlap: u64,
}

fn byte_lit(b: u8) -> String {
    format!("\\x{b:02X}")
}

/// Parses a hex string like `48 8B ?? 4? E8` into regex syntax.
fn hex_to_regex(pattern: &str) -> Result<(String, u64), Error> {
    let digits: Vec<char> = pattern
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',' && *c != '-' && *c != ':')
        .collect();
    let digits: Vec<char> = {
        // allow 0x prefixes per byte or for the whole string
        let s: String = digits.into_iter().collect();
        s.replace("0x", "").replace("0X", "").chars().collect()
    };
    if digits.is_empty() {
        return Err(Error::Parse("empty hex pattern".into()));
    }
    if digits.len() % 2 != 0 {
        return Err(Error::Parse(
            "hex pattern needs an even number of digits".into(),
        ));
    }
    let mut out = String::from("(?s-u)");
    for pair in digits.chunks(2) {
        let (h, l) = (pair[0], pair[1]);
        let wild = |c: char| c == '?' || c == '*' || c == '.';
        let nib = |c: char| c.to_digit(16).map(|d| d as u8);
        match (wild(h), wild(l)) {
            (true, true) => out.push('.'),
            (false, false) => {
                let (Some(h), Some(l)) = (nib(h), nib(l)) else {
                    return Err(Error::Parse(format!(
                        "invalid hex byte '{}{}'",
                        pair[0], pair[1]
                    )));
                };
                out.push_str(&byte_lit(h << 4 | l));
            }
            (false, true) => {
                let h = nib(h).ok_or_else(|| Error::Parse(format!("invalid hex digit '{h}'")))?;
                out.push_str(&format!(
                    "[{}-{}]",
                    byte_lit(h << 4),
                    byte_lit(h << 4 | 0xF)
                ));
            }
            (true, false) => {
                let l = nib(l).ok_or_else(|| Error::Parse(format!("invalid hex digit '{l}'")))?;
                out.push('[');
                for h in 0..16u8 {
                    out.push_str(&byte_lit(h << 4 | l));
                }
                out.push(']');
            }
        }
    }
    Ok((out, (digits.len() / 2) as u64))
}

/// Parses hex text (`DE AD be ef`, `0xDE,0xAD`) into bytes.
pub fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, Error> {
    let cleaned: String = s
        .replace("0x", "")
        .replace("0X", "")
        .replace("\\x", "")
        .chars()
        .filter(|c| {
            !c.is_whitespace() && !matches!(c, ',' | '-' | ':' | '{' | '}' | '[' | ']' | ';')
        })
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err(Error::Parse("hex needs an even number of digits".into()));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| Error::Parse(format!("invalid hex byte '{}'", &cleaned[i..i + 2])))
        })
        .collect()
}

pub fn encode_text(text: &str, enc: TextEncoding) -> Result<Vec<u8>, Error> {
    Ok(match enc {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf16le => text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect(),
        TextEncoding::Utf16be => text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect(),
        TextEncoding::Latin1 => text
            .chars()
            .map(|c| {
                u8::try_from(c as u32)
                    .map_err(|_| Error::Parse(format!("'{c}' is not a Latin-1 character")))
            })
            .collect::<Result<_, _>>()?,
    })
}

fn literal_regex(bytes: &[u8]) -> String {
    let mut s = String::from("(?s-u)");
    for &b in bytes {
        s.push_str(&byte_lit(b));
    }
    s
}

fn text_regex(text: &str, enc: TextEncoding, case_sensitive: bool) -> Result<String, Error> {
    if text.is_empty() {
        return Err(Error::Parse("empty search text".into()));
    }
    if case_sensitive {
        return Ok(literal_regex(&encode_text(text, enc)?));
    }
    if enc == TextEncoding::Utf8 {
        return Ok(format!("(?i){}", regex::escape(text)));
    }
    let mut s = String::from("(?s-u)");
    for c in text.chars() {
        let mut variants: Vec<String> = vec![c.to_string()];
        for v in c.to_lowercase().chain(c.to_uppercase()) {
            let v = v.to_string();
            if !variants.contains(&v) {
                variants.push(v);
            }
        }
        let alts: Vec<String> = variants
            .iter()
            .filter_map(|v| encode_text(v, enc).ok())
            .map(|b| b.iter().map(|&b| byte_lit(b)).collect())
            .collect();
        if alts.is_empty() {
            return Err(Error::Parse(format!("'{c}' cannot be encoded")));
        }
        s.push_str(&format!("(?:{})", alts.join("|")));
    }
    Ok(s)
}

impl Matcher {
    pub fn new(q: &Query) -> Result<Self, Error> {
        let (src, overlap) = match q {
            Query::Hex { pattern } => {
                let (re, n) = hex_to_regex(pattern)?;
                (re, n)
            }
            Query::Text {
                text,
                encoding,
                case_sensitive,
            } => {
                let n = encode_text(text, *encoding)
                    .map(|b| b.len())
                    .unwrap_or(text.len() * 4) as u64;
                (text_regex(text, *encoding, *case_sensitive)?, n * 4)
            }
            Query::Regex {
                pattern,
                case_sensitive,
            } => {
                if pattern.is_empty() {
                    return Err(Error::Parse("empty regex".into()));
                }
                let flags = if *case_sensitive { "" } else { "(?i)" };
                (format!("{flags}{pattern}"), REGEX_OVERLAP)
            }
            Query::Number {
                value,
                num_type,
                little_endian,
            } => {
                let b = encode_value(*num_type, value, *little_endian)?;
                let n = b.len() as u64;
                (literal_regex(&b), n)
            }
        };
        let re = RegexBuilder::new(&src)
            .size_limit(64 << 20)
            .build()
            .map_err(|e| Error::Parse(e.to_string()))?;
        Ok(Matcher {
            re,
            overlap: overlap.max(1),
        })
    }

    /// First match starting at or after `from`. Wraps around if `wrap`.
    pub fn find_next(&self, t: &PieceTable, from: u64, wrap: bool, task: &Task) -> Option<Match> {
        let len = t.len();
        if let Some(m) = self.scan_forward(t, from, len, task) {
            return Some(m);
        }
        if wrap && from > 0 {
            return self.scan_forward(t, 0, from.min(len), task);
        }
        None
    }

    /// Last match starting strictly before `before`. Wraps around if `wrap`.
    pub fn find_prev(&self, t: &PieceTable, before: u64, wrap: bool, task: &Task) -> Option<Match> {
        let len = t.len();
        if let Some(m) = self.scan_backward(t, 0, before.min(len), task) {
            return Some(m);
        }
        if wrap && before < len {
            return self.scan_backward(t, before, len, task);
        }
        None
    }

    /// All non-overlapping matches, up to `limit`. Returns (matches, truncated).
    pub fn find_all(&self, t: &PieceTable, limit: usize, task: &Task) -> (Vec<Match>, bool) {
        let mut out = Vec::new();
        let len = t.len();
        let mut pos = 0u64;
        let mut truncated = false;
        while pos < len {
            if task.cancelled() {
                break;
            }
            let own_end = (pos + CHUNK).min(len);
            let win_end = (own_end + self.overlap).min(len);
            let buf = t.read_vec(pos, (win_end - pos) as usize);
            let mut next_pos = own_end;
            for m in self.re.find_iter(&buf) {
                let off = pos + m.start() as u64;
                if off >= own_end {
                    break;
                }
                if m.end() == m.start() {
                    continue;
                }
                out.push(Match {
                    offset: off,
                    len: (m.end() - m.start()) as u64,
                });
                // continue after a match that extends into the overlap
                next_pos = next_pos.max(pos + m.end() as u64);
                if out.len() >= limit {
                    truncated = true;
                    return (out, truncated);
                }
            }
            task.report(own_end as f64 / len as f64);
            pos = next_pos;
        }
        (out, truncated)
    }

    fn scan_forward(&self, t: &PieceTable, from: u64, until: u64, task: &Task) -> Option<Match> {
        let len = t.len();
        let mut pos = from;
        while pos < until {
            if task.cancelled() {
                return None;
            }
            let own_end = (pos + CHUNK).min(until);
            let win_end = (own_end + self.overlap).min(len);
            let buf = t.read_vec(pos, (win_end - pos) as usize);
            for m in self.re.find_iter(&buf) {
                let off = pos + m.start() as u64;
                if off >= own_end {
                    break;
                }
                if m.end() > m.start() {
                    return Some(Match {
                        offset: off,
                        len: (m.end() - m.start()) as u64,
                    });
                }
            }
            task.report((own_end - from) as f64 / (until - from).max(1) as f64);
            pos = own_end;
        }
        None
    }

    fn scan_backward(&self, t: &PieceTable, from: u64, until: u64, task: &Task) -> Option<Match> {
        let len = t.len();
        let mut end = until;
        while end > from {
            if task.cancelled() {
                return None;
            }
            let own_start = end.saturating_sub(CHUNK).max(from);
            let win_end = (end + self.overlap).min(len);
            let buf = t.read_vec(own_start, (win_end - own_start) as usize);
            let mut last = None;
            let mut i = 0usize;
            // overlapping search so the closest match before `end` is found
            while i < buf.len() {
                let Some(m) = self.re.find_at(&buf, i) else {
                    break;
                };
                let off = own_start + m.start() as u64;
                if off >= end {
                    break;
                }
                if m.end() > m.start() {
                    last = Some(Match {
                        offset: off,
                        len: (m.end() - m.start()) as u64,
                    });
                }
                i = m.start() + 1;
            }
            if last.is_some() {
                return last;
            }
            task.report((until - own_start) as f64 / (until - from).max(1) as f64);
            end = own_start;
        }
        None
    }
}

/// Parses the replacement for a query into bytes (same notation as the query).
pub fn replacement_bytes(q: &Query, replacement: &str) -> Result<Vec<u8>, Error> {
    match q {
        Query::Hex { .. } => parse_hex_bytes(replacement),
        Query::Text { encoding, .. } => encode_text(replacement, *encoding),
        Query::Regex { .. } => Ok(replacement.as_bytes().to_vec()),
        Query::Number {
            num_type,
            little_endian,
            ..
        } => encode_value(*num_type, replacement, *little_endian),
    }
}

/// Replaces all given matches with `with` as a single undo step.
pub fn replace_all(t: &mut PieceTable, matches: &[Match], with: &[u8]) {
    if matches.is_empty() {
        return;
    }
    t.begin(matches[0].offset);
    for m in matches.iter().rev() {
        t.replace(m.offset, m.len, with);
    }
    t.commit();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Source;

    fn t(d: &[u8]) -> PieceTable {
        PieceTable::new(Source::Mem(d.to_vec()))
    }

    #[test]
    fn hex_with_wildcards() {
        let tb = t(b"\x00\x48\x8B\x05\x10\xE8\x48\x8B\x00\x00\xE8");
        let m = Matcher::new(&Query::Hex {
            pattern: "48 8B ?? ?? E8".into(),
        })
        .unwrap();
        let (all, _) = m.find_all(&tb, 100, &Task::none());
        assert_eq!(
            all,
            vec![Match { offset: 1, len: 5 }, Match { offset: 6, len: 5 }]
        );
        let m = Matcher::new(&Query::Hex {
            pattern: "8B 0?".into(),
        })
        .unwrap();
        assert_eq!(
            m.find_next(&tb, 0, false, &Task::none()),
            Some(Match { offset: 2, len: 2 })
        );
        assert_eq!(
            m.find_next(&tb, 3, false, &Task::none()),
            Some(Match { offset: 7, len: 2 })
        );
        assert_eq!(
            m.find_prev(&tb, 7, false, &Task::none()),
            Some(Match { offset: 2, len: 2 })
        );
        assert_eq!(
            m.find_next(&tb, 8, true, &Task::none()),
            Some(Match { offset: 2, len: 2 })
        );
    }

    #[test]
    fn text_utf16_case_insensitive() {
        let data: Vec<u8> = "xxHeLLo"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let tb = t(&data);
        let m = Matcher::new(&Query::Text {
            text: "hello".into(),
            encoding: TextEncoding::Utf16le,
            case_sensitive: false,
        })
        .unwrap();
        assert_eq!(
            m.find_next(&tb, 0, false, &Task::none()),
            Some(Match { offset: 4, len: 10 })
        );
    }

    #[test]
    fn number_search() {
        let tb = t(&[0, 0xEF, 0xBE, 0xAD, 0xDE, 0]);
        let m = Matcher::new(&Query::Number {
            value: "0xDEADBEEF".into(),
            num_type: NumType::U32,
            little_endian: true,
        })
        .unwrap();
        assert_eq!(
            m.find_next(&tb, 0, false, &Task::none()),
            Some(Match { offset: 1, len: 4 })
        );
    }

    #[test]
    fn matches_across_chunk_border() {
        let mut data = vec![0u8; (CHUNK + 10) as usize];
        let at = CHUNK as usize - 2;
        data[at..at + 4].copy_from_slice(b"ABCD");
        let tb = t(&data);
        let m = Matcher::new(&Query::Text {
            text: "ABCD".into(),
            encoding: TextEncoding::Utf8,
            case_sensitive: true,
        })
        .unwrap();
        let (all, _) = m.find_all(&tb, 10, &Task::none());
        assert_eq!(
            all,
            vec![Match {
                offset: at as u64,
                len: 4
            }]
        );
        assert_eq!(
            m.find_prev(&tb, tb.len(), false, &Task::none())
                .map(|m| m.offset),
            Some(at as u64)
        );
    }

    #[test]
    fn replace_all_single_undo() {
        let mut tb = t(b"a-b-c");
        let m = Matcher::new(&Query::Text {
            text: "-".into(),
            encoding: TextEncoding::Utf8,
            case_sensitive: true,
        })
        .unwrap();
        let (all, _) = m.find_all(&tb, 100, &Task::none());
        replace_all(&mut tb, &all, b"::");
        assert_eq!(tb.read_vec(0, 100), b"a::b::c");
        tb.undo();
        assert_eq!(tb.read_vec(0, 100), b"a-b-c");
    }

    #[test]
    fn parse_hex() {
        assert_eq!(
            parse_hex_bytes("DE ad 0xBE,0xEF").unwrap(),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        );
        assert!(parse_hex_bytes("ABC").is_err());
    }
}
