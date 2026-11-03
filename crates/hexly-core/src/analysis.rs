//! Whole-file analysis helpers: strings, entropy map, hashes, file compare.

use crate::buffer::PieceTable;
use crate::Task;
use serde::{Deserialize, Serialize};

const CHUNK: u64 = 4 << 20;

// ---- strings ----------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct FoundString {
    pub offset: u64,
    /// Length in bytes.
    pub len: u64,
    pub encoding: &'static str,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringsOptions {
    pub min_len: usize,
    pub ascii: bool,
    pub utf16: bool,
    pub limit: usize,
}

fn printable(b: u8) -> bool {
    (0x20..0x7F).contains(&b) || b == b'\t'
}

const MAX_STRING: usize = 4096;

/// Extracts printable ASCII and UTF-16LE strings.
pub fn strings(t: &PieceTable, o: StringsOptions, task: &Task) -> (Vec<FoundString>, bool) {
    let mut out = Vec::new();
    let min = o.min_len.max(2);
    // ASCII state
    let mut a_start = 0u64;
    let mut a_buf: Vec<u8> = Vec::new();
    // UTF-16LE state: two phases (even/odd alignment)
    let mut u_start = [0u64; 2];
    let mut u_buf: [Vec<u8>; 2] = [Vec::new(), Vec::new()];
    let mut u_pending: [Option<u8>; 2] = [None, None];
    let len = t.len();
    let mut truncated = false;

    let flush_a = |out: &mut Vec<FoundString>, start: u64, buf: &mut Vec<u8>, full_len: u64| {
        if buf.len() >= min {
            out.push(FoundString {
                offset: start,
                len: full_len,
                encoding: "ascii",
                text: String::from_utf8_lossy(buf).into_owned(),
            });
        }
        buf.clear();
    };

    let mut a_count = 0u64;
    let mut u_count = [0u64; 2];
    let mut pos = 0u64;
    'outer: while pos < len {
        if task.cancelled() {
            break;
        }
        let chunk = t.read_vec(pos, CHUNK as usize);
        for (i, &b) in chunk.iter().enumerate() {
            let off = pos + i as u64;
            if o.ascii {
                if printable(b) {
                    if a_count == 0 {
                        a_start = off;
                    }
                    if a_buf.len() < MAX_STRING {
                        a_buf.push(b);
                    }
                    a_count += 1;
                } else if a_count > 0 {
                    flush_a(&mut out, a_start, &mut a_buf, a_count);
                    a_count = 0;
                }
            }
            if o.utf16 {
                let phase = (off & 1) as usize;
                if phase == 0 {
                    u_pending[0] = Some(b);
                    // close the odd-aligned unit that starts at off-1
                    if let Some(lo) = u_pending[1].take() {
                        step_u16(
                            &mut out,
                            &mut u_start[1],
                            &mut u_buf[1],
                            &mut u_count[1],
                            lo,
                            b,
                            off - 1,
                            min,
                        );
                    }
                } else {
                    u_pending[1] = Some(b);
                    if let Some(lo) = u_pending[0].take() {
                        step_u16(
                            &mut out,
                            &mut u_start[0],
                            &mut u_buf[0],
                            &mut u_count[0],
                            lo,
                            b,
                            off - 1,
                            min,
                        );
                    }
                }
            }
            if out.len() >= o.limit {
                truncated = true;
                break 'outer;
            }
        }
        pos += chunk.len() as u64;
        task.report(pos as f64 / len.max(1) as f64);
    }
    if !truncated {
        if a_count > 0 {
            flush_a(&mut out, a_start, &mut a_buf, a_count);
        }
        for p in 0..2 {
            if u_count[p] >= min as u64 {
                out.push(FoundString {
                    offset: u_start[p],
                    len: u_count[p] * 2,
                    encoding: "utf16le",
                    text: String::from_utf8_lossy(&u_buf[p]).into_owned(),
                });
            }
        }
    }
    out.sort_by_key(|s| s.offset);
    // ASCII strings of length 1 inside UTF-16 runs are filtered by min length already.
    out.truncate(o.limit);
    (out, truncated)
}

#[allow(clippy::too_many_arguments)]
fn step_u16(
    out: &mut Vec<FoundString>,
    start: &mut u64,
    buf: &mut Vec<u8>,
    count: &mut u64,
    lo: u8,
    hi: u8,
    off: u64,
    min: usize,
) {
    if hi == 0 && printable(lo) {
        if *count == 0 {
            *start = off;
        }
        if buf.len() < MAX_STRING {
            buf.push(lo);
        }
        *count += 1;
    } else if *count > 0 {
        if *count >= min as u64 {
            out.push(FoundString {
                offset: *start,
                len: *count * 2,
                encoding: "utf16le",
                text: String::from_utf8_lossy(buf).into_owned(),
            });
        }
        buf.clear();
        *count = 0;
    }
}

// ---- entropy map ------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntropyMap {
    /// Bytes covered by each block.
    pub block_size: u64,
    /// Shannon entropy per block, scaled 0..=255.
    pub entropy: Vec<u8>,
    /// Dominant byte class per block: 0 zero, 1 text, 2 binary, 3 high-entropy.
    pub class: Vec<u8>,
}

fn entropy_of(counts: &[u32; 256], n: u32) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let n = n as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum::<f64>()
        / 8.0
}

/// Computes per-block entropy. Large files are sampled (up to 16 KiB per block).
pub fn entropy_map(t: &PieceTable, blocks: usize, task: &Task) -> EntropyMap {
    let len = t.len();
    let blocks = blocks.clamp(1, 1 << 16) as u64;
    let block_size = len.div_ceil(blocks).max(1);
    let sample = block_size.min(16 << 10) as usize;
    let mut ent = Vec::new();
    let mut class = Vec::new();
    let mut off = 0u64;
    while off < len {
        if task.cancelled() {
            break;
        }
        let bytes = t.read_vec(off, sample);
        let mut counts = [0u32; 256];
        for &b in &bytes {
            counts[b as usize] += 1;
        }
        let n = bytes.len() as u32;
        let e = entropy_of(&counts, n);
        let text: u32 =
            (0x20..0x7F).map(|i| counts[i]).sum::<u32>() + counts[9] + counts[10] + counts[13];
        let c = if counts[0] == n {
            0
        } else if e > 0.9 {
            3
        } else if text * 10 >= n * 9 {
            1
        } else {
            2
        };
        ent.push((e * 255.0).round() as u8);
        class.push(c);
        off += block_size;
        task.report(off as f64 / len as f64);
    }
    EntropyMap {
        block_size,
        entropy: ent,
        class,
    }
}

// ---- hashes -----------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct HashResult {
    pub name: &'static str,
    pub value: String,
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn hashes(t: &PieceTable, offset: u64, len: u64, task: &Task) -> Vec<HashResult> {
    use sha1::Digest as _;
    let mut crc = crc32fast::Hasher::new();
    let mut md5 = md5::Md5::new();
    let mut s1 = sha1::Sha1::new();
    let mut s256 = sha2::Sha256::new();
    let mut s512 = sha2::Sha512::new();
    let mut xx64 = xxhash_rust::xxh64::Xxh64::new(0);
    let mut xx3 = xxhash_rust::xxh3::Xxh3::new();
    let mut adler_a: u32 = 1;
    let mut adler_b: u32 = 0;
    let end = (offset + len).min(t.len());
    let total = end.saturating_sub(offset).max(1);
    let mut done = 0u64;
    let mut pos = offset;
    while pos < end {
        if task.cancelled() {
            return vec![];
        }
        let n = (end - pos).min(CHUNK);
        t.for_each_slice(pos, n, |_, s| {
            crc.update(s);
            md5.update(s);
            s1.update(s);
            s256.update(s);
            s512.update(s);
            xx64.update(s);
            xx3.update(s);
            for part in s.chunks(5552) {
                for &b in part {
                    adler_a += b as u32;
                    adler_b += adler_a;
                }
                adler_a %= 65521;
                adler_b %= 65521;
            }
            true
        });
        pos += n;
        done += n;
        task.report(done as f64 / total as f64);
    }
    vec![
        HashResult {
            name: "CRC32",
            value: format!("{:08x}", crc.finalize()),
        },
        HashResult {
            name: "Adler-32",
            value: format!("{:08x}", (adler_b << 16) | adler_a),
        },
        HashResult {
            name: "MD5",
            value: hexs(&md5.finalize()),
        },
        HashResult {
            name: "SHA-1",
            value: hexs(&s1.finalize()),
        },
        HashResult {
            name: "SHA-256",
            value: hexs(&s256.finalize()),
        },
        HashResult {
            name: "SHA-512",
            value: hexs(&s512.finalize()),
        },
        HashResult {
            name: "XXH64",
            value: format!("{:016x}", xx64.digest()),
        },
        HashResult {
            name: "XXH3-64",
            value: format!("{:016x}", xx3.digest()),
        },
    ]
}

// ---- compare ----------------------------------------------------------------

/// Finds the next offset at or after `from` where `a` and `b` differ
/// (including where one file ends before the other).
pub fn next_difference(a: &PieceTable, b: &PieceTable, from: u64, task: &Task) -> Option<u64> {
    let common = a.len().min(b.len());
    let mut pos = from;
    while pos < common {
        if task.cancelled() {
            return None;
        }
        let n = (common - pos).min(CHUNK) as usize;
        let x = a.read_vec(pos, n);
        let y = b.read_vec(pos, n);
        if let Some(i) = x.iter().zip(&y).position(|(p, q)| p != q) {
            return Some(pos + i as u64);
        }
        pos += n as u64;
        task.report(pos as f64 / common.max(1) as f64);
    }
    (a.len() != b.len() && from.max(common) < a.len().max(b.len())).then_some(from.max(common))
}

/// Finds the last offset strictly before `before` where `a` and `b` differ.
pub fn prev_difference(a: &PieceTable, b: &PieceTable, before: u64, task: &Task) -> Option<u64> {
    let maxlen = a.len().max(b.len());
    let common = a.len().min(b.len());
    let before = before.min(maxlen);
    if before > common {
        return Some(before - 1);
    }
    let mut end = before;
    while end > 0 {
        if task.cancelled() {
            return None;
        }
        let start = end.saturating_sub(CHUNK);
        let n = (end - start) as usize;
        let x = a.read_vec(start, n);
        let y = b.read_vec(start, n);
        if let Some(i) = x.iter().zip(&y).rposition(|(p, q)| p != q) {
            return Some(start + i as u64);
        }
        end = start;
    }
    None
}

/// Counts differing bytes (positional compare).
pub fn count_differences(a: &PieceTable, b: &PieceTable, task: &Task) -> u64 {
    let common = a.len().min(b.len());
    let mut diff = a.len().max(b.len()) - common;
    let mut pos = 0;
    while pos < common {
        if task.cancelled() {
            break;
        }
        let n = (common - pos).min(CHUNK) as usize;
        let x = a.read_vec(pos, n);
        let y = b.read_vec(pos, n);
        diff += x.iter().zip(&y).filter(|(p, q)| p != q).count() as u64;
        pos += n as u64;
        task.report(pos as f64 / common.max(1) as f64);
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Source;

    fn t(d: &[u8]) -> PieceTable {
        PieceTable::new(Source::Mem(d.to_vec()))
    }

    #[test]
    fn finds_strings() {
        let mut d = b"\x00\x01hello\x00ab\x01".to_vec();
        d.extend("wide!".encode_utf16().flat_map(|u| u.to_le_bytes()));
        d.push(0xFF);
        let (s, _) = strings(
            &t(&d),
            StringsOptions {
                min_len: 4,
                ascii: true,
                utf16: true,
                limit: 100,
            },
            &Task::none(),
        );
        let texts: Vec<(&str, u64, &str)> = s
            .iter()
            .map(|x| (x.text.as_str(), x.offset, x.encoding))
            .collect();
        assert_eq!(texts, vec![("hello", 2, "ascii"), ("wide!", 11, "utf16le")]);
    }

    #[test]
    fn hash_values() {
        let h = hashes(&t(b"abc"), 0, 3, &Task::none());
        let get = |n: &str| h.iter().find(|x| x.name == n).unwrap().value.clone();
        assert_eq!(get("MD5"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            get("SHA-256"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(get("CRC32"), "352441c2");
        assert_eq!(get("Adler-32"), "024d0127");
    }

    #[test]
    fn differences() {
        let a = t(b"abcdef");
        let b = t(b"abXdeYgh");
        assert_eq!(next_difference(&a, &b, 0, &Task::none()), Some(2));
        assert_eq!(next_difference(&a, &b, 3, &Task::none()), Some(5));
        assert_eq!(next_difference(&a, &b, 6, &Task::none()), Some(6));
        assert_eq!(prev_difference(&a, &b, 5, &Task::none()), Some(2));
        assert_eq!(prev_difference(&a, &b, 2, &Task::none()), None);
        assert_eq!(count_differences(&a, &b, &Task::none()), 4);
        let e = entropy_map(&t(&[0u8; 1000]), 10, &Task::none());
        assert_eq!(e.entropy.len(), 10);
        assert!(e.class.iter().all(|&c| c == 0));
    }
}
