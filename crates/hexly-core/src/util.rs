//! Copy-as formatting and the go-to expression evaluator.

use crate::Error;
use base64::Engine as _;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CopyFormat {
    Hex,
    HexCompact,
    Text,
    C,
    Rust,
    Python,
    Go,
    Java,
    Base64,
    EscapedString,
}

/// Maximum selection size accepted for clipboard copy.
pub const MAX_COPY: u64 = 64 << 20;

pub fn format_bytes(b: &[u8], fmt: CopyFormat, upper: bool) -> String {
    let h = |x: &u8| {
        if upper {
            format!("{x:02X}")
        } else {
            format!("{x:02x}")
        }
    };
    let list = |prefix: &str, per_line: usize, indent: &str| -> String {
        b.chunks(per_line)
            .map(|c| {
                format!(
                    "{indent}{}",
                    c.iter()
                        .map(|x| format!("{prefix}{}", h(x)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    };
    match fmt {
        CopyFormat::Hex => b.iter().map(h).collect::<Vec<_>>().join(" "),
        CopyFormat::HexCompact => b.iter().map(h).collect(),
        CopyFormat::Text => String::from_utf8_lossy(b).into_owned(),
        CopyFormat::C => format!(
            "unsigned char data[{}] = {{\n{}\n}};",
            b.len(),
            list("0x", 12, "    ")
        ),
        CopyFormat::Rust => format!(
            "let data: [u8; {}] = [\n{}\n];",
            b.len(),
            list("0x", 12, "    ")
        ),
        CopyFormat::Go => format!("data := []byte{{\n{},\n}}", list("0x", 12, "\t")),
        CopyFormat::Java => format!(
            "byte[] data = {{\n{}\n}};",
            b.chunks(12)
                .map(|c| format!(
                    "    {}",
                    c.iter()
                        .map(|&x| if x < 0x80 {
                            format!("0x{}", h(&x))
                        } else {
                            format!("(byte) 0x{}", h(&x))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .collect::<Vec<_>>()
                .join(",\n")
        ),
        CopyFormat::Python => {
            let mut s = String::from("data = b\"");
            for &x in b {
                s.push_str(&format!("\\x{}", h(&x)));
            }
            s.push('"');
            s
        }
        CopyFormat::EscapedString => {
            let mut s = String::new();
            for &x in b {
                match x {
                    b'\\' => s.push_str("\\\\"),
                    b'"' => s.push_str("\\\""),
                    b'\n' => s.push_str("\\n"),
                    b'\r' => s.push_str("\\r"),
                    b'\t' => s.push_str("\\t"),
                    0x20..=0x7E => s.push(x as char),
                    _ => s.push_str(&format!("\\x{}", h(&x))),
                }
            }
            s
        }
        CopyFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(b),
    }
}

/// Decodes pasted text into bytes: hex in any common notation, or base64 as a fallback.
pub fn parse_paste(text: &str, as_hex: bool) -> Result<Vec<u8>, Error> {
    if !as_hex {
        return Ok(text.as_bytes().to_vec());
    }
    let plain = crate::search::parse_hex_bytes(text);
    if plain.is_ok() {
        return plain;
    }
    // array/literal syntax: `unsigned char x[4] = { ... };`, `data = b"\x00"`, `[]byte{...}`
    if let Some(eq) = text.find('=') {
        let body = text[eq + 1..]
            .replace("[]byte", "")
            .replace("(byte)", "")
            .replace("b\"", "")
            .replace('"', "");
        if let Ok(b) = crate::search::parse_hex_bytes(&body) {
            if !b.is_empty() {
                return Ok(b);
            }
        }
    }
    base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|_| Error::Parse("clipboard does not contain hex bytes".into()))
}

// ---- expressions --------------------------------------------------------------

/// Evaluates `0x400 + 16*3`, `end - 0x20`, `cur + 4`, `sym:main` style expressions.
/// Numbers are decimal unless prefixed (0x, 0b, 0o) or suffixed with `h`.
/// Bare hex like `1F00` is accepted when `hex_default` is set.
pub fn eval_expr(s: &str, vars: &HashMap<String, i128>, hex_default: bool) -> Result<i128, Error> {
    let tokens = lex(s, hex_default, vars)?;
    let mut p = Parser { t: &tokens, i: 0 };
    let v = p.expr()?;
    if p.i != tokens.len() {
        return Err(Error::Parse("unexpected input after expression".into()));
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i128),
    Op(char),
}

fn lex(s: &str, hex_default: bool, vars: &HashMap<String, i128>) -> Result<Vec<Tok>, Error> {
    let c: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if "+-*/%()&|^~<>".contains(ch) {
            if (ch == '<' || ch == '>') && c.get(i + 1) == Some(&ch) {
                out.push(Tok::Op(if ch == '<' { 'L' } else { 'R' }));
                i += 2;
                continue;
            }
            if ch == '<' || ch == '>' {
                return Err(Error::Parse("use << or >> for shifts".into()));
            }
            out.push(Tok::Op(ch));
            i += 1;
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' || ch == ':' || ch == '.' || ch == '$' || ch == '@' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || "_:.$@?".contains(c[i])) {
                i += 1;
            }
            let word: String = c[start..i].iter().collect();
            let lw = word.to_ascii_lowercase().replace('_', "");
            let num = if let Some(h) = lw.strip_prefix("0x") {
                i128::from_str_radix(h, 16).ok()
            } else if let Some(b) = lw.strip_prefix("0b") {
                i128::from_str_radix(b, 2).ok()
            } else if let Some(o) = lw.strip_prefix("0o") {
                i128::from_str_radix(o, 8).ok()
            } else if lw.ends_with('h')
                && lw.len() > 1
                && lw[..lw.len() - 1].chars().all(|c| c.is_ascii_hexdigit())
            {
                i128::from_str_radix(&lw[..lw.len() - 1], 16).ok()
            } else if lw.chars().all(|c| c.is_ascii_digit()) {
                if hex_default {
                    i128::from_str_radix(&lw, 16).ok()
                } else {
                    lw.parse().ok()
                }
            } else if hex_default
                && lw.chars().all(|c| c.is_ascii_hexdigit())
                && !vars.contains_key(&lw)
            {
                i128::from_str_radix(&lw, 16).ok()
            } else {
                None
            };
            match num {
                Some(n) => out.push(Tok::Num(n)),
                None => {
                    let key = word
                        .strip_prefix("sym:")
                        .map(|s| format!("sym:{s}"))
                        .unwrap_or(lw.clone());
                    match vars.get(&key).or_else(|| vars.get(&word)) {
                        Some(v) => out.push(Tok::Num(*v)),
                        None => return Err(Error::Parse(format!("unknown name '{word}'"))),
                    }
                }
            }
            continue;
        }
        return Err(Error::Parse(format!("unexpected '{ch}'")));
    }
    Ok(out)
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn op(&mut self, ops: &str) -> Option<char> {
        if let Some(Tok::Op(c)) = self.peek() {
            if ops.contains(*c) {
                let c = *c;
                self.i += 1;
                return Some(c);
            }
        }
        None
    }
    // precedence: | ^ & << >> + - * / %
    fn expr(&mut self) -> Result<i128, Error> {
        let mut v = self.xor()?;
        while self.op("|").is_some() {
            v |= self.xor()?;
        }
        Ok(v)
    }
    fn xor(&mut self) -> Result<i128, Error> {
        let mut v = self.and()?;
        while self.op("^").is_some() {
            v ^= self.and()?;
        }
        Ok(v)
    }
    fn and(&mut self) -> Result<i128, Error> {
        let mut v = self.shift()?;
        while self.op("&").is_some() {
            v &= self.shift()?;
        }
        Ok(v)
    }
    fn shift(&mut self) -> Result<i128, Error> {
        let mut v = self.sum()?;
        while let Some(o) = self.op("LR") {
            let r = self.sum()?;
            if !(0..127).contains(&r) {
                return Err(Error::Parse("shift out of range".into()));
            }
            v = if o == 'L' { v << r } else { v >> r };
        }
        Ok(v)
    }
    fn sum(&mut self) -> Result<i128, Error> {
        let mut v = self.term()?;
        while let Some(o) = self.op("+-") {
            let r = self.term()?;
            v = if o == '+' {
                v.checked_add(r)
            } else {
                v.checked_sub(r)
            }
            .ok_or_else(|| Error::Parse("overflow".into()))?;
        }
        Ok(v)
    }
    fn term(&mut self) -> Result<i128, Error> {
        let mut v = self.unary()?;
        while let Some(o) = self.op("*/%") {
            let r = self.unary()?;
            v = match o {
                '*' => v.checked_mul(r),
                '/' => v.checked_div(r),
                _ => v.checked_rem(r),
            }
            .ok_or_else(|| {
                Error::Parse(if r == 0 {
                    "division by zero".into()
                } else {
                    "overflow".into()
                })
            })?;
        }
        Ok(v)
    }
    fn unary(&mut self) -> Result<i128, Error> {
        match self.op("-+~") {
            Some('-') => Ok(-self.unary()?),
            Some('~') => Ok(!self.unary()?),
            Some(_) => self.unary(),
            None => self.atom(),
        }
    }
    fn atom(&mut self) -> Result<i128, Error> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(n)
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let v = self.expr()?;
                if self.op(")").is_none() {
                    return Err(Error::Parse("missing ')'".into()));
                }
                Ok(v)
            }
            _ => Err(Error::Parse("expected a number".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        let mut vars = HashMap::new();
        vars.insert("end".into(), 1000);
        vars.insert("sym:main".into(), 0x4000);
        let e = |s: &str| eval_expr(s, &vars, false).unwrap();
        assert_eq!(e("0x400 + 16*3"), 0x430);
        assert_eq!(e("end - 0x20"), 968);
        assert_eq!(e("(1 + 2) * 3"), 9);
        assert_eq!(e("1Fh"), 31);
        assert_eq!(e("1 << 4 | 1"), 17);
        assert_eq!(e("sym:main + 4"), 0x4004);
        assert_eq!(eval_expr("1F00", &vars, true).unwrap(), 0x1F00);
        assert!(eval_expr("1/0", &vars, false).is_err());
        assert!(eval_expr("nope", &vars, false).is_err());
    }

    #[test]
    fn copy_and_paste() {
        let b = [0xDE, 0xAD, 0x00, 0x41];
        assert_eq!(format_bytes(&b, CopyFormat::Hex, true), "DE AD 00 41");
        assert_eq!(
            format_bytes(&b, CopyFormat::Python, false),
            "data = b\"\\xde\\xad\\x00\\x41\""
        );
        for f in [
            CopyFormat::Hex,
            CopyFormat::C,
            CopyFormat::Rust,
            CopyFormat::Python,
            CopyFormat::Go,
            CopyFormat::Java,
            CopyFormat::HexCompact,
            CopyFormat::Base64,
        ] {
            let s = format_bytes(&b, f, false);
            let back = parse_paste(&s, true).unwrap_or_else(|e| panic!("{f:?}: {s} -> {e}"));
            assert_eq!(back, b, "{f:?}: {s}");
        }
    }
}
