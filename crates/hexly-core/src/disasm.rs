//! Disassembly via Capstone.

use crate::Error;
use capstone::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X86,
    #[serde(rename = "x86_64")]
    X86_64,
    Arm,
    Thumb,
    Arm64,
    RiscV32,
    RiscV64,
    Mips32,
    Mips32Le,
    Mips64,
    Mips64Le,
    Ppc32,
    Ppc64,
    Ppc64Le,
}

impl Arch {
    pub const ALL: [Arch; 14] = [
        Arch::X86,
        Arch::X86_64,
        Arch::Arm,
        Arch::Thumb,
        Arch::Arm64,
        Arch::RiscV32,
        Arch::RiscV64,
        Arch::Mips32,
        Arch::Mips32Le,
        Arch::Mips64,
        Arch::Mips64Le,
        Arch::Ppc32,
        Arch::Ppc64,
        Arch::Ppc64Le,
    ];

    /// Upper bound of a single instruction's size.
    pub fn max_insn(self) -> usize {
        match self {
            Arch::X86 | Arch::X86_64 => 15,
            _ => 4,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Insn {
    pub offset: u64,
    pub address: u64,
    pub size: u16,
    pub bytes: String,
    pub mnemonic: String,
    pub operands: String,
    /// Branch/call target address if the operand is a plain immediate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u64>,
}

fn engine(arch: Arch) -> Result<Capstone, capstone::Error> {
    use capstone::arch::*;
    let cs = match arch {
        Arch::X86 => Capstone::new().x86().mode(x86::ArchMode::Mode32).build(),
        Arch::X86_64 => Capstone::new().x86().mode(x86::ArchMode::Mode64).build(),
        Arch::Arm => Capstone::new().arm().mode(arm::ArchMode::Arm).build(),
        Arch::Thumb => Capstone::new().arm().mode(arm::ArchMode::Thumb).build(),
        Arch::Arm64 => Capstone::new().arm64().mode(arm64::ArchMode::Arm).build(),
        Arch::RiscV32 => Capstone::new()
            .riscv()
            .mode(riscv::ArchMode::RiscV32)
            .extra_mode([riscv::ArchExtraMode::RiscVC].into_iter())
            .build(),
        Arch::RiscV64 => Capstone::new()
            .riscv()
            .mode(riscv::ArchMode::RiscV64)
            .extra_mode([riscv::ArchExtraMode::RiscVC].into_iter())
            .build(),
        Arch::Mips32 => Capstone::new()
            .mips()
            .mode(mips::ArchMode::Mips32)
            .endian(capstone::Endian::Big)
            .build(),
        Arch::Mips32Le => Capstone::new()
            .mips()
            .mode(mips::ArchMode::Mips32)
            .endian(capstone::Endian::Little)
            .build(),
        Arch::Mips64 => Capstone::new()
            .mips()
            .mode(mips::ArchMode::Mips64)
            .endian(capstone::Endian::Big)
            .build(),
        Arch::Mips64Le => Capstone::new()
            .mips()
            .mode(mips::ArchMode::Mips64)
            .endian(capstone::Endian::Little)
            .build(),
        Arch::Ppc32 => Capstone::new()
            .ppc()
            .mode(ppc::ArchMode::Mode32)
            .endian(capstone::Endian::Big)
            .build(),
        Arch::Ppc64 => Capstone::new()
            .ppc()
            .mode(ppc::ArchMode::Mode64)
            .endian(capstone::Endian::Big)
            .build(),
        Arch::Ppc64Le => Capstone::new()
            .ppc()
            .mode(ppc::ArchMode::Mode64)
            .endian(capstone::Endian::Little)
            .build(),
    }?;
    Ok(cs)
}

fn parse_target(ops: &str) -> Option<u64> {
    let t = ops.trim().trim_start_matches('#');
    let h = t.strip_prefix("0x")?;
    u64::from_str_radix(h, 16).ok()
}

fn is_branch(mnemonic: &str) -> bool {
    let m = mnemonic.to_ascii_lowercase();
    m.starts_with('j')
        || m.starts_with("call")
        || m == "b"
        || m == "bl"
        || m.starts_with("b.")
        || m.starts_with("cb")
        || m.starts_with("tb")
        || m == "jal"
        || m.starts_with("beq")
        || m.starts_with("bne")
        || m.starts_with("loop")
}

/// Disassembles up to `count` instructions from `bytes`, which start at file
/// offset `offset` and virtual address `address`. Undecodable bytes are shown
/// as `.byte` so the listing never stalls.
pub fn disassemble(
    arch: Arch,
    bytes: &[u8],
    offset: u64,
    address: u64,
    count: usize,
) -> Result<Vec<Insn>, Error> {
    let cs = engine(arch).map_err(|e| Error::Other(e.to_string()))?;
    let step = match arch {
        Arch::X86 | Arch::X86_64 => 1,
        Arch::Thumb | Arch::RiscV32 | Arch::RiscV64 => 2,
        _ => 4,
    };
    let mut out = Vec::with_capacity(count);
    let mut pos = 0usize;
    while out.len() < count && pos < bytes.len() {
        let rest = &bytes[pos..];
        let insns = cs.disasm_count(rest, address + pos as u64, 1);
        let decoded = insns.ok().and_then(|list| {
            list.iter().next().map(|i| {
                let mnemonic = i.mnemonic().unwrap_or("").to_string();
                let operands = i.op_str().unwrap_or("").to_string();
                let target = if is_branch(&mnemonic) {
                    parse_target(&operands)
                } else {
                    None
                };
                (i.bytes().len(), mnemonic, operands, target)
            })
        });
        let (size, mnemonic, operands, target) = match decoded {
            Some(d) if d.0 > 0 => d,
            _ => {
                let n = step.min(rest.len());
                (
                    n,
                    ".byte".into(),
                    rest[..n]
                        .iter()
                        .map(|b| format!("0x{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    None,
                )
            }
        };
        out.push(Insn {
            offset: offset + pos as u64,
            address: address + pos as u64,
            size: size as u16,
            bytes: rest[..size]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
            mnemonic,
            operands,
            target,
        });
        pos += size;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x86_64() {
        // push rbp; mov rbp, rsp; call +0; ret
        let code = [0x55, 0x48, 0x89, 0xE5, 0xE8, 0x00, 0x00, 0x00, 0x00, 0xC3];
        let r = disassemble(Arch::X86_64, &code, 0x100, 0x1000, 10).unwrap();
        let text: Vec<String> = r
            .iter()
            .map(|i| format!("{} {}", i.mnemonic, i.operands))
            .collect();
        assert_eq!(text, ["push rbp", "mov rbp, rsp", "call 0x1009", "ret "]);
        assert_eq!(r[2].target, Some(0x1009));
        assert_eq!(r[1].offset, 0x101);
    }

    #[test]
    fn arm64_and_garbage() {
        // ret ; then 4 bytes of something undecodable
        let code = [0xC0, 0x03, 0x5F, 0xD6, 0xFF, 0xFF, 0xFF, 0xFF];
        let r = disassemble(Arch::Arm64, &code, 0, 0, 10).unwrap();
        assert_eq!(r[0].mnemonic, "ret");
        assert_eq!(r.len(), 2);
    }
}
