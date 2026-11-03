//! Data inspector: interprets the bytes at the cursor as many different types.

use crate::Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NumType {
    U8,
    I8,
    U16,
    I16,
    U24,
    U32,
    I32,
    U64,
    I64,
    F16,
    Bf16,
    F32,
    F64,
    Uleb128,
    Sleb128,
    Binary,
}

impl NumType {
    pub fn size(self) -> usize {
        match self {
            NumType::U8 | NumType::I8 | NumType::Binary => 1,
            NumType::U16 | NumType::I16 | NumType::F16 | NumType::Bf16 => 2,
            NumType::U24 => 3,
            NumType::U32 | NumType::I32 | NumType::F32 => 4,
            NumType::U64 | NumType::I64 | NumType::F64 => 8,
            NumType::Uleb128 | NumType::Sleb128 => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Field {
    /// Stable id; also the NumType name when the field is editable.
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    /// Number of bytes this interpretation covers (for highlighting).
    pub size: usize,
    pub editable: bool,
}

fn le_u(b: &[u8], n: usize, le: bool) -> Option<u64> {
    let s = b.get(..n)?;
    let mut v = 0u64;
    for i in 0..n {
        let byte = if le { s[i] } else { s[n - 1 - i] };
        v |= (byte as u64) << (8 * i);
    }
    Some(v)
}

pub fn read_uleb(b: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for (i, &byte) in b.iter().enumerate().take(10) {
        v |= ((byte & 0x7F) as u64) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

pub fn read_sleb(b: &[u8]) -> Option<(i64, usize)> {
    let mut v = 0i64;
    let mut shift = 0;
    for (i, &byte) in b.iter().enumerate().take(10) {
        v |= ((byte & 0x7F) as i64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            if shift < 64 && byte & 0x40 != 0 {
                v |= -1i64 << shift;
            }
            return Some((v, i + 1));
        }
    }
    None
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn format_unix(secs: i64, nanos: u32) -> Option<String> {
    // keep within a sane range (year 0..=9999)
    if !(-62_167_219_200..=253_402_300_799).contains(&secs) {
        return None;
    }
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    let frac = if nanos > 0 {
        format!(".{:03}", nanos / 1_000_000)
    } else {
        String::new()
    };
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}{frac} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

fn fmt_float(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 {
            "+Inf".into()
        } else {
            "-Inf".into()
        }
    } else if v != 0.0 && (v.abs() >= 1e16 || v.abs() < 1e-6) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

fn guid(b: &[u8]) -> Option<String> {
    let b = b.get(..16)?;
    Some(format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{}}}",
        u32::from_le_bytes(b[0..4].try_into().ok()?),
        u16::from_le_bytes(b[4..6].try_into().ok()?),
        u16::from_le_bytes(b[6..8].try_into().ok()?),
        b[8],
        b[9],
        b[10..16]
            .iter()
            .map(|x| format!("{x:02X}"))
            .collect::<String>()
    ))
}

/// Interprets `b` (bytes starting at the cursor) in every supported way.
pub fn inspect(b: &[u8], le: bool) -> Vec<Field> {
    let mut f = Vec::new();
    let mut push = |key: &'static str,
                    label: &'static str,
                    value: Option<String>,
                    size: usize,
                    editable: bool| {
        f.push(Field {
            key,
            label,
            value: value.unwrap_or_else(|| "—".into()),
            size,
            editable,
        });
    };
    let first = b.first().copied();
    push(
        "binary",
        "Binary",
        first.map(|x| format!("{x:08b}")),
        1,
        true,
    );
    push("u8", "uint8", first.map(|x| x.to_string()), 1, true);
    push("i8", "int8", first.map(|x| (x as i8).to_string()), 1, true);
    push(
        "u16",
        "uint16",
        le_u(b, 2, le).map(|v| v.to_string()),
        2,
        true,
    );
    push(
        "i16",
        "int16",
        le_u(b, 2, le).map(|v| (v as u16 as i16).to_string()),
        2,
        true,
    );
    push(
        "u24",
        "uint24",
        le_u(b, 3, le).map(|v| v.to_string()),
        3,
        true,
    );
    push(
        "u32",
        "uint32",
        le_u(b, 4, le).map(|v| v.to_string()),
        4,
        true,
    );
    push(
        "i32",
        "int32",
        le_u(b, 4, le).map(|v| (v as u32 as i32).to_string()),
        4,
        true,
    );
    push(
        "u64",
        "uint64",
        le_u(b, 8, le).map(|v| v.to_string()),
        8,
        true,
    );
    push(
        "i64",
        "int64",
        le_u(b, 8, le).map(|v| (v as i64).to_string()),
        8,
        true,
    );
    push(
        "f16",
        "float16",
        le_u(b, 2, le).map(|v| fmt_float(half::f16::from_bits(v as u16).to_f64())),
        2,
        true,
    );
    push(
        "bf16",
        "bfloat16",
        le_u(b, 2, le).map(|v| fmt_float(half::bf16::from_bits(v as u16).to_f64())),
        2,
        true,
    );
    push(
        "f32",
        "float32",
        le_u(b, 4, le).map(|v| fmt_float(f32::from_bits(v as u32) as f64)),
        4,
        true,
    );
    push(
        "f64",
        "float64",
        le_u(b, 8, le).map(|v| fmt_float(f64::from_bits(v))),
        8,
        true,
    );
    let uleb = read_uleb(b);
    push(
        "uleb128",
        "ULEB128",
        uleb.map(|(v, _)| v.to_string()),
        uleb.map_or(1, |x| x.1),
        true,
    );
    let sleb = read_sleb(b);
    push(
        "sleb128",
        "SLEB128",
        sleb.map(|(v, _)| v.to_string()),
        sleb.map_or(1, |x| x.1),
        true,
    );
    push(
        "ascii",
        "ASCII",
        first.map(|x| {
            if x.is_ascii_graphic() || x == b' ' {
                format!("'{}'", x as char)
            } else {
                format!("\\x{x:02X}")
            }
        }),
        1,
        false,
    );
    let utf8 = (1..=4.min(b.len())).find_map(|n| {
        std::str::from_utf8(&b[..n])
            .ok()
            .map(|s| (s.to_string(), n))
    });
    push(
        "utf8",
        "UTF-8",
        utf8.as_ref().map(|(s, _)| {
            format!(
                "'{}' U+{:04X}",
                s.escape_debug(),
                s.chars().next().unwrap() as u32
            )
        }),
        utf8.map_or(1, |x| x.1),
        false,
    );
    let utf16 = le_u(b, 2, le).and_then(|u| char::from_u32(u as u32));
    push(
        "utf16",
        "UTF-16",
        utf16.map(|c| format!("'{}' U+{:04X}", c.escape_debug(), c as u32)),
        2,
        false,
    );
    push(
        "unix32",
        "time_t (32)",
        le_u(b, 4, le).and_then(|v| format_unix(v as u32 as i32 as i64, 0)),
        4,
        false,
    );
    push(
        "unix64",
        "time_t (64)",
        le_u(b, 8, le).and_then(|v| format_unix(v as i64, 0)),
        8,
        false,
    );
    push(
        "filetime",
        "FILETIME",
        le_u(b, 8, le).and_then(|v| {
            let secs = (v / 10_000_000) as i64 - 11_644_473_600;
            format_unix(secs, ((v % 10_000_000) * 100) as u32)
        }),
        8,
        false,
    );
    push(
        "dostime",
        "DOS date/time",
        le_u(b, 4, le).and_then(|v| {
            let time = v as u16;
            let date = (v >> 16) as u16;
            let (y, m, d) = (1980 + (date >> 9) as u32, (date >> 5) & 0xF, date & 0x1F);
            if !(1..=12).contains(&m) || d == 0 {
                return None;
            }
            Some(format!(
                "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
                time >> 11,
                (time >> 5) & 0x3F,
                (time & 0x1F) * 2
            ))
        }),
        4,
        false,
    );
    push("guid", "GUID", guid(b), 16, false);
    f
}

fn parse_int(s: &str) -> Result<i128, Error> {
    let s = s.trim().replace('_', "");
    let (neg, body) = match s.strip_prefix('-') {
        Some(r) => (true, r.trim()),
        None => (false, s.strip_prefix('+').unwrap_or(&s)),
    };
    let lower = body.to_ascii_lowercase();
    let v = if let Some(h) = lower.strip_prefix("0x") {
        i128::from_str_radix(h, 16)
    } else if let Some(h) = lower.strip_suffix('h') {
        i128::from_str_radix(h, 16)
    } else if let Some(b) = lower.strip_prefix("0b") {
        i128::from_str_radix(b, 2)
    } else if let Some(o) = lower.strip_prefix("0o") {
        i128::from_str_radix(o, 8)
    } else {
        lower.parse::<i128>()
    }
    .map_err(|_| Error::Parse(format!("'{s}' is not a number")))?;
    Ok(if neg { -v } else { v })
}

fn ranged(v: i128, min: i128, max: i128, name: &str) -> Result<u64, Error> {
    if v < min || v > max {
        return Err(Error::Parse(format!("{v} does not fit in {name}")));
    }
    Ok(v as u64)
}

fn to_bytes(v: u64, n: usize, le: bool) -> Vec<u8> {
    let mut out: Vec<u8> = v.to_le_bytes()[..n].to_vec();
    if !le {
        out.reverse();
    }
    out
}

fn parse_float(s: &str) -> Result<f64, Error> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "nan" => Ok(f64::NAN),
        "inf" | "+inf" | "infinity" => Ok(f64::INFINITY),
        "-inf" | "-infinity" => Ok(f64::NEG_INFINITY),
        _ => t
            .parse()
            .map_err(|_| Error::Parse(format!("'{s}' is not a number"))),
    }
}

/// Encodes a textual value as bytes of the given type.
pub fn encode_value(ty: NumType, s: &str, le: bool) -> Result<Vec<u8>, Error> {
    Ok(match ty {
        NumType::Binary => {
            let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            let t = t.strip_prefix("0b").unwrap_or(&t);
            vec![u8::from_str_radix(t, 2)
                .map_err(|_| Error::Parse(format!("'{s}' is not an 8-bit binary number")))?]
        }
        NumType::U8 => to_bytes(ranged(parse_int(s)?, 0, 0xFF, "uint8")?, 1, le),
        NumType::I8 => to_bytes(ranged(parse_int(s)?, -128, 0xFF, "int8")?, 1, le),
        NumType::U16 => to_bytes(ranged(parse_int(s)?, 0, 0xFFFF, "uint16")?, 2, le),
        NumType::I16 => to_bytes(ranged(parse_int(s)?, -0x8000, 0xFFFF, "int16")?, 2, le),
        NumType::U24 => to_bytes(ranged(parse_int(s)?, 0, 0xFF_FFFF, "uint24")?, 3, le),
        NumType::U32 => to_bytes(ranged(parse_int(s)?, 0, 0xFFFF_FFFF, "uint32")?, 4, le),
        NumType::I32 => to_bytes(
            ranged(parse_int(s)?, -0x8000_0000, 0xFFFF_FFFF, "int32")?,
            4,
            le,
        ),
        NumType::U64 => to_bytes(ranged(parse_int(s)?, 0, u64::MAX as i128, "uint64")?, 8, le),
        NumType::I64 => to_bytes(
            ranged(parse_int(s)?, i64::MIN as i128, u64::MAX as i128, "int64")?,
            8,
            le,
        ),
        NumType::F16 => to_bytes(half::f16::from_f64(parse_float(s)?).to_bits() as u64, 2, le),
        NumType::Bf16 => to_bytes(
            half::bf16::from_f64(parse_float(s)?).to_bits() as u64,
            2,
            le,
        ),
        NumType::F32 => to_bytes((parse_float(s)? as f32).to_bits() as u64, 4, le),
        NumType::F64 => to_bytes(parse_float(s)?.to_bits(), 8, le),
        NumType::Uleb128 => {
            let mut v = ranged(parse_int(s)?, 0, u64::MAX as i128, "ULEB128")?;
            let mut out = vec![];
            loop {
                let byte = (v & 0x7F) as u8;
                v >>= 7;
                if v == 0 {
                    out.push(byte);
                    break;
                }
                out.push(byte | 0x80);
            }
            out
        }
        NumType::Sleb128 => {
            let mut v =
                ranged(parse_int(s)?, i64::MIN as i128, i64::MAX as i128, "SLEB128")? as i64;
            let mut out = vec![];
            loop {
                let byte = (v & 0x7F) as u8;
                v >>= 7;
                let done = (v == 0 && byte & 0x40 == 0) || (v == -1 && byte & 0x40 != 0);
                out.push(if done { byte } else { byte | 0x80 });
                if done {
                    break;
                }
            }
            out
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(fields: &[Field], key: &str) -> String {
        fields.iter().find(|f| f.key == key).unwrap().value.clone()
    }

    #[test]
    fn integers_and_endianness() {
        let b = [0xEF, 0xBE, 0xAD, 0xDE, 0, 0, 0, 0];
        let le = inspect(&b, true);
        assert_eq!(get(&le, "u32"), "3735928559");
        assert_eq!(get(&le, "i32"), "-559038737");
        assert_eq!(get(&le, "u16"), "48879");
        let be = inspect(&b, false);
        assert_eq!(get(&be, "u16"), "61374");
        assert_eq!(get(&le, "binary"), "11101111");
    }

    #[test]
    fn floats_and_times() {
        let f = inspect(&1.5f32.to_le_bytes(), true);
        assert_eq!(get(&f, "f32"), "1.5");
        let t = inspect(&0u32.to_le_bytes(), true);
        assert_eq!(get(&t, "unix32"), "1970-01-01 00:00:00 UTC");
        let t = inspect(&1_700_000_000u32.to_le_bytes(), true);
        assert_eq!(get(&t, "unix32"), "2023-11-14 22:13:20 UTC");
        // 2024-01-01 00:00:00 as FILETIME
        let ft: u64 = (1_704_067_200 + 11_644_473_600) * 10_000_000;
        assert_eq!(
            get(&inspect(&ft.to_le_bytes(), true), "filetime"),
            "2024-01-01 00:00:00 UTC"
        );
    }

    #[test]
    fn leb128_roundtrip() {
        for v in [0i64, 1, 63, 64, -1, -64, -65, 300, i64::MAX, i64::MIN] {
            let b = encode_value(NumType::Sleb128, &v.to_string(), true).unwrap();
            assert_eq!(read_sleb(&b), Some((v, b.len())), "sleb {v}");
        }
        for v in [0u64, 127, 128, 624485, u64::MAX] {
            let b = encode_value(NumType::Uleb128, &v.to_string(), true).unwrap();
            assert_eq!(read_uleb(&b), Some((v, b.len())), "uleb {v}");
        }
        assert_eq!(
            encode_value(NumType::Uleb128, "624485", true).unwrap(),
            vec![0xE5, 0x8E, 0x26]
        );
    }

    #[test]
    fn encode() {
        assert_eq!(
            encode_value(NumType::U32, "0xDEADBEEF", true).unwrap(),
            vec![0xEF, 0xBE, 0xAD, 0xDE]
        );
        assert_eq!(
            encode_value(NumType::I16, "-2", false).unwrap(),
            vec![0xFF, 0xFE]
        );
        assert!(encode_value(NumType::U8, "256", true).is_err());
        assert_eq!(
            encode_value(NumType::Binary, "1010 0101", true).unwrap(),
            vec![0xA5]
        );
        assert_eq!(
            encode_value(NumType::U16, "1Fh", true).unwrap(),
            vec![0x1F, 0]
        );
    }
}
