#![no_main]
//! Parsing arbitrary bytes as ELF/PE/Mach-O must never panic.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let info = hexly_core::formats::analyze(data);
    if let Some(arch) = info.arch {
        let start = info.entry_offset.unwrap_or(0) as usize;
        if start < data.len() {
            let _ = hexly_core::disasm::disassemble(arch, &data[start..], start as u64, 0, 32);
        }
    }
});
