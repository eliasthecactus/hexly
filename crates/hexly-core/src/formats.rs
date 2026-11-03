//! Binary format detection and parsing (ELF, PE, Mach-O and fat Mach-O),
//! plus magic-number detection for common non-executable formats.

use crate::disasm::Arch;
use goblin::{elf, mach, pe, Object};
use serde::Serialize;

const MAX_SYMBOLS: usize = 50_000;

#[derive(Clone, Debug, Serialize, Default)]
pub struct Node {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

impl Node {
    fn group(label: impl Into<String>, children: Vec<Node>) -> Node {
        Node {
            label: label.into(),
            value: Some(children.len().to_string()),
            children,
            ..Default::default()
        }
    }
    fn field(label: impl Into<String>, value: impl Into<String>) -> Node {
        Node {
            label: label.into(),
            value: Some(value.into()),
            ..Default::default()
        }
    }
    fn at(mut self, offset: u64, size: u64) -> Node {
        self.offset = Some(offset);
        self.size = Some(size);
        self
    }
}

/// A file range with a name, drawn as an overlay in the hex view.
#[derive(Clone, Debug, Serialize)]
pub struct Region {
    pub name: String,
    pub kind: &'static str,
    pub offset: u64,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vaddr: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<String>,
}

/// Mapping of a virtual address range to a file range.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Mapping {
    pub vaddr: u64,
    pub vsize: u64,
    pub offset: u64,
    pub fsize: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Symbol {
    pub name: String,
    pub vaddr: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    pub size: u64,
    pub kind: &'static str,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FormatInfo {
    pub kind: String,
    pub description: String,
    pub arch: Option<Arch>,
    pub entry: Option<u64>,
    pub entry_offset: Option<u64>,
    pub regions: Vec<Region>,
    pub mappings: Vec<Mapping>,
    pub symbols: Vec<Symbol>,
    pub tree: Vec<Node>,
}

impl FormatInfo {
    pub fn va_to_offset(&self, va: u64) -> Option<u64> {
        va_to_offset(&self.mappings, va)
    }
    pub fn offset_to_va(&self, off: u64) -> Option<u64> {
        offset_to_va(&self.mappings, off)
    }
}

pub fn va_to_offset(maps: &[Mapping], va: u64) -> Option<u64> {
    maps.iter()
        .filter(|m| m.fsize > 0)
        .find(|m| va >= m.vaddr && va < m.vaddr + m.fsize.min(m.vsize.max(m.fsize)))
        .map(|m| m.offset + (va - m.vaddr))
}

pub fn offset_to_va(maps: &[Mapping], off: u64) -> Option<u64> {
    maps.iter()
        .filter(|m| m.fsize > 0 && m.vsize > 0)
        .find(|m| off >= m.offset && off < m.offset + m.fsize)
        .map(|m| m.vaddr + (off - m.offset))
}

fn hex(v: u64) -> String {
    format!("0x{v:X}")
}

fn magic_kind(b: &[u8]) -> Option<(&'static str, &'static str)> {
    let table: &[(&[u8], usize, &str, &str)] = &[
        (b"\x89PNG\r\n\x1a\n", 0, "PNG", "PNG image"),
        (b"\xFF\xD8\xFF", 0, "JPEG", "JPEG image"),
        (b"GIF8", 0, "GIF", "GIF image"),
        (b"%PDF-", 0, "PDF", "PDF document"),
        (
            b"PK\x03\x04",
            0,
            "ZIP",
            "ZIP archive (also JAR, APK, DOCX, IPA…)",
        ),
        (b"PK\x05\x06", 0, "ZIP", "ZIP archive (empty)"),
        (b"\x1F\x8B", 0, "gzip", "gzip compressed data"),
        (b"BZh", 0, "bzip2", "bzip2 compressed data"),
        (b"\xFD7zXZ\x00", 0, "xz", "xz compressed data"),
        (b"\x28\xB5\x2F\xFD", 0, "zstd", "Zstandard compressed data"),
        (b"7z\xBC\xAF\x27\x1C", 0, "7z", "7-Zip archive"),
        (b"Rar!\x1A\x07", 0, "RAR", "RAR archive"),
        (b"SQLite format 3\x00", 0, "SQLite", "SQLite 3 database"),
        (b"\x00asm", 0, "WASM", "WebAssembly module"),
        (
            b"\xCA\xFE\xBA\xBE\x00\x00\x00",
            0,
            "Java class",
            "Java class file",
        ),
        (b"dex\n", 0, "DEX", "Android Dalvik executable"),
        (b"bplist", 0, "bplist", "Binary property list"),
        (b"RIFF", 0, "RIFF", "RIFF container (WAV, AVI, WebP…)"),
        (b"OggS", 0, "Ogg", "Ogg container"),
        (b"fLaC", 0, "FLAC", "FLAC audio"),
        (b"ID3", 0, "MP3", "MP3 audio with ID3 tag"),
        (b"ftyp", 4, "MP4", "ISO media (MP4, MOV, HEIC…)"),
        (b"\x1A\x45\xDF\xA3", 0, "Matroska", "Matroska/WebM video"),
        (b"ustar", 257, "tar", "tar archive"),
        (b"!<arch>\n", 0, "ar", "ar archive / static library"),
        (b"\xED\xAB\xEE\xDB", 0, "RPM", "RPM package"),
        (b"MSCF", 0, "CAB", "Microsoft Cabinet archive"),
        (
            b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
            0,
            "OLE",
            "OLE compound file (DOC, XLS, MSI…)",
        ),
        (b"\x7FELF", 0, "ELF", "ELF (unparseable)"),
        (b"BM", 0, "BMP", "BMP image"),
        (b"II*\x00", 0, "TIFF", "TIFF image (little endian)"),
        (b"MM\x00*", 0, "TIFF", "TIFF image (big endian)"),
        (b"\x00\x00\x01\x00", 0, "ICO", "Windows icon"),
        (b"wOFF", 0, "WOFF", "WOFF font"),
        (b"wOF2", 0, "WOFF2", "WOFF2 font"),
        (b"\x00\x01\x00\x00\x00", 0, "TTF", "TrueType font"),
        (b"OTTO", 0, "OTF", "OpenType font"),
        (b"\x04\x22\x4D\x18", 0, "LZ4", "LZ4 frame"),
    ];
    table
        .iter()
        .find(|(m, at, _, _)| b.len() >= at + m.len() && &b[*at..at + m.len()] == *m)
        .map(|(_, _, k, d)| (*k, *d))
}

/// Parses `data` (the whole file) and returns what was recognised.
pub fn analyze(data: &[u8]) -> FormatInfo {
    let parsed = match Object::parse(data) {
        Ok(Object::Elf(e)) => Some(parse_elf(&e)),
        Ok(Object::PE(p)) => Some(parse_pe(&p, data)),
        Ok(Object::Mach(mach::Mach::Binary(m))) => Some(parse_macho(&m, 0)),
        Ok(Object::Mach(mach::Mach::Fat(f))) => Some(parse_fat(&f)),
        _ => None,
    };
    if let Some(mut info) = parsed {
        info.symbols.sort_by_key(|s| s.vaddr);
        info.regions
            .sort_by_key(|r| (r.offset, std::cmp::Reverse(r.size)));
        return info;
    }
    match magic_kind(data) {
        Some((k, d)) => FormatInfo {
            kind: k.into(),
            description: d.into(),
            ..Default::default()
        },
        None if data.is_empty() => FormatInfo {
            kind: "Empty".into(),
            description: "Empty file".into(),
            ..Default::default()
        },
        None => {
            let sample = &data[..data.len().min(4096)];
            let printable = sample
                .iter()
                .filter(|b| b.is_ascii_graphic() || b.is_ascii_whitespace())
                .count();
            let text = printable * 100 >= sample.len() * 95;
            FormatInfo {
                kind: if text { "Text" } else { "Data" }.into(),
                description: if text {
                    "Text file"
                } else {
                    "Unknown binary data"
                }
                .into(),
                ..Default::default()
            }
        }
    }
}

// ---- ELF ------------------------------------------------------------------

fn elf_arch(e: &elf::Elf) -> Option<Arch> {
    use elf::header::*;
    Some(match e.header.e_machine {
        EM_X86_64 => Arch::X86_64,
        EM_386 => Arch::X86,
        EM_AARCH64 => Arch::Arm64,
        EM_ARM => {
            if e.entry & 1 == 1 {
                Arch::Thumb
            } else {
                Arch::Arm
            }
        }
        EM_RISCV => {
            if e.is_64 {
                Arch::RiscV64
            } else {
                Arch::RiscV32
            }
        }
        EM_MIPS => {
            if e.little_endian {
                Arch::Mips32Le
            } else {
                Arch::Mips32
            }
        }
        EM_PPC => Arch::Ppc32,
        EM_PPC64 => {
            if e.little_endian {
                Arch::Ppc64Le
            } else {
                Arch::Ppc64
            }
        }
        _ => return None,
    })
}

fn elf_flags(f: u32) -> String {
    use elf::program_header::{PF_R, PF_W, PF_X};
    format!(
        "{}{}{}",
        if f & PF_R != 0 { 'r' } else { '-' },
        if f & PF_W != 0 { 'w' } else { '-' },
        if f & PF_X != 0 { 'x' } else { '-' }
    )
}

fn parse_elf(e: &elf::Elf) -> FormatInfo {
    use elf::{header, program_header, section_header};
    let h = &e.header;
    let mut info = FormatInfo {
        kind: "ELF".into(),
        arch: elf_arch(e),
        ..Default::default()
    };
    let class = if e.is_64 { "64-bit" } else { "32-bit" };
    let endian = if e.little_endian { "LSB" } else { "MSB" };
    info.description = format!(
        "ELF {class} {endian} {}, {}",
        header::et_to_str(h.e_type),
        header::machine_to_str(h.e_machine)
    );

    for ph in &e.program_headers {
        info.mappings.push(Mapping {
            vaddr: ph.p_vaddr,
            vsize: ph.p_memsz,
            offset: ph.p_offset,
            fsize: ph.p_filesz,
        });
    }
    info.mappings.retain(|m| m.vsize > 0);
    // PT_LOAD first so lookups prefer real load segments
    let loads: Vec<Mapping> = e
        .program_headers
        .iter()
        .filter(|p| p.p_type == program_header::PT_LOAD)
        .map(|ph| Mapping {
            vaddr: ph.p_vaddr,
            vsize: ph.p_memsz,
            offset: ph.p_offset,
            fsize: ph.p_filesz,
        })
        .collect();
    if !loads.is_empty() {
        info.mappings = loads;
    }

    if h.e_entry != 0 {
        info.entry = Some(h.e_entry);
        info.entry_offset = va_to_offset(&info.mappings, h.e_entry & !1);
    }

    let ehsize = h.e_ehsize as u64;
    info.regions.push(Region {
        name: "ELF header".into(),
        kind: "header",
        offset: 0,
        size: ehsize,
        vaddr: None,
        flags: None,
    });
    let mut hdr = vec![
        Node::field("Class", class),
        Node::field(
            "Data",
            if e.little_endian {
                "little endian"
            } else {
                "big endian"
            },
        ),
        Node::field("Type", header::et_to_str(h.e_type)),
        Node::field("Machine", header::machine_to_str(h.e_machine)),
        Node::field("Entry point", hex(h.e_entry)),
        Node::field(
            "Program headers",
            format!("{} at {}", h.e_phnum, hex(h.e_phoff)),
        ),
        Node::field(
            "Section headers",
            format!("{} at {}", h.e_shnum, hex(h.e_shoff)),
        ),
    ];
    if let Some(i) = e.interpreter {
        hdr.push(Node::field("Interpreter", i));
    }
    info.tree.push(
        Node {
            children: hdr,
            ..Node::group("ELF header", vec![])
        }
        .at(0, ehsize),
    );

    let mut phs = vec![];
    for (i, ph) in e.program_headers.iter().enumerate() {
        let name = program_header::pt_to_str(ph.p_type);
        phs.push(
            Node {
                label: format!("[{i}] {name}"),
                value: Some(format!(
                    "{} off {} size {} va {}",
                    elf_flags(ph.p_flags),
                    hex(ph.p_offset),
                    hex(ph.p_filesz),
                    hex(ph.p_vaddr)
                )),
                ..Default::default()
            }
            .at(ph.p_offset, ph.p_filesz),
        );
        if ph.p_type == program_header::PT_LOAD && ph.p_filesz > 0 {
            info.regions.push(Region {
                name: format!("LOAD[{i}]"),
                kind: "segment",
                offset: ph.p_offset,
                size: ph.p_filesz,
                vaddr: Some(ph.p_vaddr),
                flags: Some(elf_flags(ph.p_flags)),
            });
        }
    }
    if h.e_phnum > 0 {
        let size = h.e_phnum as u64 * h.e_phentsize as u64;
        info.tree
            .push(Node::group("Program headers", phs).at(h.e_phoff, size));
    }

    let mut shs = vec![];
    for (i, sh) in e.section_headers.iter().enumerate() {
        let name = e.shdr_strtab.get_at(sh.sh_name).unwrap_or("");
        let in_file = sh.sh_type != section_header::SHT_NOBITS;
        let mut n = Node {
            label: format!("[{i}] {}", if name.is_empty() { "<null>" } else { name }),
            value: Some(format!(
                "{} off {} size {}{}",
                section_header::sht_to_str(sh.sh_type),
                hex(sh.sh_offset),
                hex(sh.sh_size),
                if sh.sh_addr != 0 {
                    format!(" va {}", hex(sh.sh_addr))
                } else {
                    String::new()
                }
            )),
            ..Default::default()
        };
        if in_file && sh.sh_size > 0 {
            n = n.at(sh.sh_offset, sh.sh_size);
            info.regions.push(Region {
                name: name.to_string(),
                kind: "section",
                offset: sh.sh_offset,
                size: sh.sh_size,
                vaddr: (sh.sh_addr != 0).then_some(sh.sh_addr),
                flags: None,
            });
        }
        shs.push(n);
    }
    if h.e_shnum > 0 {
        let size = h.e_shnum as u64 * h.e_shentsize as u64;
        info.tree
            .push(Node::group("Sections", shs).at(h.e_shoff, size));
        info.regions.push(Region {
            name: "Section headers".into(),
            kind: "header",
            offset: h.e_shoff,
            size,
            vaddr: None,
            flags: None,
        });
    }

    let push_syms = |syms: &elf::Symtab,
                     strtab: &goblin::strtab::Strtab,
                     kind: &'static str,
                     info: &mut FormatInfo| {
        for s in syms.iter() {
            if info.symbols.len() >= MAX_SYMBOLS {
                break;
            }
            let name = strtab.get_at(s.st_name).unwrap_or("");
            if name.is_empty() || s.st_value == 0 {
                continue;
            }
            let k = if s.is_function() {
                "function"
            } else if s.st_type() == elf::sym::STT_OBJECT {
                "object"
            } else {
                kind
            };
            info.symbols.push(Symbol {
                name: name.to_string(),
                vaddr: s.st_value,
                offset: va_to_offset(&info.mappings, s.st_value & !1),
                size: s.st_size,
                kind: k,
            });
        }
    };
    push_syms(&e.syms, &e.strtab, "symbol", &mut info);
    push_syms(&e.dynsyms, &e.dynstrtab, "dynamic", &mut info);
    info.symbols
        .sort_by(|a, b| (a.vaddr, &a.name).cmp(&(b.vaddr, &b.name)));
    info.symbols
        .dedup_by(|a, b| a.vaddr == b.vaddr && a.name == b.name);

    if !e.libraries.is_empty() {
        info.tree.push(Node::group(
            "Libraries",
            e.libraries.iter().map(|l| Node::field(*l, "")).collect(),
        ));
    }
    let imports: Vec<Node> = e
        .dynsyms
        .iter()
        .filter(|s| s.is_import())
        .filter_map(|s| e.dynstrtab.get_at(s.st_name))
        .filter(|n| !n.is_empty())
        .map(|n| Node::field(n, ""))
        .collect();
    if !imports.is_empty() {
        info.tree.push(Node::group("Imports", imports));
    }
    info
}

// ---- PE -------------------------------------------------------------------

fn pe_arch(machine: u16) -> Option<Arch> {
    use pe::header::*;
    Some(match machine {
        COFF_MACHINE_X86_64 => Arch::X86_64,
        COFF_MACHINE_X86 => Arch::X86,
        COFF_MACHINE_ARM64 => Arch::Arm64,
        COFF_MACHINE_ARMNT => Arch::Thumb,
        COFF_MACHINE_ARM => Arch::Arm,
        _ => return None,
    })
}

fn pe_machine_name(machine: u16) -> String {
    use pe::header::*;
    match machine {
        COFF_MACHINE_X86_64 => "x86-64".into(),
        COFF_MACHINE_X86 => "i386".into(),
        COFF_MACHINE_ARM64 => "ARM64".into(),
        COFF_MACHINE_ARMNT => "ARM Thumb-2".into(),
        COFF_MACHINE_ARM => "ARM".into(),
        m => format!("machine {m:#06x}"),
    }
}

fn parse_pe(p: &pe::PE, data: &[u8]) -> FormatInfo {
    use pe::section_table::*;
    let coff = &p.header.coff_header;
    let base = p.image_base;
    let mut info = FormatInfo {
        kind: "PE".into(),
        arch: pe_arch(coff.machine),
        ..Default::default()
    };
    let what = if p.is_lib { "DLL" } else { "executable" };
    let subsystem = p
        .header
        .optional_header
        .map(|o| match o.windows_fields.subsystem {
            2 => "GUI",
            3 => "console",
            10 => "EFI application",
            1 => "native",
            _ => "other",
        })
        .unwrap_or("object");
    info.description = format!(
        "PE{} {what} ({subsystem}), {}",
        if p.is_64 { "32+" } else { "32" },
        pe_machine_name(coff.machine)
    );

    // Headers map 1:1 at the image base.
    let headers_size = p
        .header
        .optional_header
        .map(|o| o.windows_fields.size_of_headers as u64)
        .unwrap_or(0);
    if headers_size > 0 {
        info.mappings.push(Mapping {
            vaddr: base,
            vsize: headers_size,
            offset: 0,
            fsize: headers_size,
        });
    }
    let mut secs = vec![];
    for s in &p.sections {
        let name = s.name().unwrap_or("?").to_string();
        let ch = s.characteristics;
        let flags = format!(
            "{}{}{}",
            if ch & IMAGE_SCN_MEM_READ != 0 {
                'r'
            } else {
                '-'
            },
            if ch & IMAGE_SCN_MEM_WRITE != 0 {
                'w'
            } else {
                '-'
            },
            if ch & IMAGE_SCN_MEM_EXECUTE != 0 {
                'x'
            } else {
                '-'
            }
        );
        let va = base + s.virtual_address as u64;
        info.mappings.push(Mapping {
            vaddr: va,
            vsize: (s.virtual_size as u64).max(s.size_of_raw_data as u64),
            offset: s.pointer_to_raw_data as u64,
            fsize: s.size_of_raw_data as u64,
        });
        let mut n = Node {
            label: name.clone(),
            value: Some(format!(
                "{flags} off {} size {} va {}",
                hex(s.pointer_to_raw_data as u64),
                hex(s.size_of_raw_data as u64),
                hex(va)
            )),
            ..Default::default()
        };
        if s.size_of_raw_data > 0 {
            n = n.at(s.pointer_to_raw_data as u64, s.size_of_raw_data as u64);
            info.regions.push(Region {
                name,
                kind: "section",
                offset: s.pointer_to_raw_data as u64,
                size: s.size_of_raw_data as u64,
                vaddr: Some(va),
                flags: Some(flags),
            });
        }
        secs.push(n);
    }

    let pe_off = p.header.dos_header.pe_pointer as u64;
    info.regions.push(Region {
        name: "DOS header".into(),
        kind: "header",
        offset: 0,
        size: 64,
        vaddr: None,
        flags: None,
    });
    info.tree.push(
        Node {
            children: vec![
                Node::field("Magic", "MZ"),
                Node::field("PE header offset", hex(pe_off)),
            ],
            ..Node::group("DOS header", vec![])
        }
        .at(0, 64),
    );
    let mut ch = vec![
        Node::field("Machine", pe_machine_name(coff.machine)),
        Node::field("Sections", coff.number_of_sections.to_string()),
        Node::field(
            "Timestamp",
            crate::inspector::format_unix(coff.time_date_stamp as i64, 0).unwrap_or_default(),
        ),
        Node::field("Characteristics", hex(coff.characteristics as u64)),
    ];
    if let Some(o) = p.header.optional_header {
        let w = &o.windows_fields;
        ch.push(Node::field("Image base", hex(base)));
        ch.push(Node::field(
            "Entry point (RVA)",
            hex(o.standard_fields.address_of_entry_point as u64),
        ));
        ch.push(Node::field("Subsystem", subsystem));
        ch.push(Node::field("Size of image", hex(w.size_of_image as u64)));
        ch.push(Node::field(
            "DLL characteristics",
            hex(w.dll_characteristics as u64),
        ));
        ch.push(Node::field("Checksum", hex(w.check_sum as u64)));
    }
    info.tree.push(
        Node {
            children: ch,
            ..Node::group("PE header", vec![])
        }
        .at(pe_off, 24 + coff.size_of_optional_header as u64),
    );
    info.regions.push(Region {
        name: "PE header".into(),
        kind: "header",
        offset: pe_off,
        size: 24 + coff.size_of_optional_header as u64,
        vaddr: None,
        flags: None,
    });
    info.tree.push(Node::group("Sections", secs));

    if p.entry != 0 {
        let va = base + p.entry as u64;
        info.entry = Some(va);
        info.entry_offset = va_to_offset(&info.mappings, va);
    }

    let mut imports: std::collections::BTreeMap<String, Vec<Node>> = Default::default();
    for imp in &p.imports {
        let va = base + imp.rva as u64;
        imports
            .entry(imp.dll.to_string())
            .or_default()
            .push(Node::field(
                imp.name.to_string(),
                format!("IAT {}", hex(va)),
            ));
        if info.symbols.len() < MAX_SYMBOLS {
            info.symbols.push(Symbol {
                name: format!("{}!{}", imp.dll, imp.name),
                vaddr: va,
                offset: va_to_offset(&info.mappings, va),
                size: if p.is_64 { 8 } else { 4 },
                kind: "import",
            });
        }
    }
    if !imports.is_empty() {
        info.tree.push(Node::group(
            "Imports",
            imports
                .into_iter()
                .map(|(dll, fns)| Node::group(dll, fns))
                .collect(),
        ));
    }
    let mut exports = vec![];
    for ex in &p.exports {
        let name = ex.name.unwrap_or("<ordinal>").to_string();
        let va = base + ex.rva as u64;
        exports.push(Node::field(name.clone(), hex(va)));
        if info.symbols.len() < MAX_SYMBOLS {
            info.symbols.push(Symbol {
                name,
                vaddr: va,
                offset: va_to_offset(&info.mappings, va),
                size: ex.size as u64,
                kind: "export",
            });
        }
    }
    if !exports.is_empty() {
        info.tree.push(Node::group("Exports", exports));
    }
    // overlay: data after the last section (installers, signatures, appended payloads)
    let end = p
        .sections
        .iter()
        .map(|s| s.pointer_to_raw_data as u64 + s.size_of_raw_data as u64)
        .max()
        .unwrap_or(0);
    if end > 0 && (end as usize) < data.len() {
        info.regions.push(Region {
            name: "Overlay".into(),
            kind: "overlay",
            offset: end,
            size: data.len() as u64 - end,
            vaddr: None,
            flags: None,
        });
        info.tree.push(
            Node::field("Overlay", format!("{} bytes", data.len() as u64 - end))
                .at(end, data.len() as u64 - end),
        );
    }
    info
}

// ---- Mach-O -----------------------------------------------------------------

fn mach_arch(cputype: u32) -> Option<Arch> {
    use mach::cputype::*;
    Some(match cputype {
        CPU_TYPE_X86_64 => Arch::X86_64,
        CPU_TYPE_X86 => Arch::X86,
        CPU_TYPE_ARM64 | CPU_TYPE_ARM64_32 => Arch::Arm64,
        CPU_TYPE_ARM => Arch::Arm,
        CPU_TYPE_POWERPC => Arch::Ppc32,
        CPU_TYPE_POWERPC64 => Arch::Ppc64,
        _ => return None,
    })
}

fn mach_arch_name(cputype: u32, sub: u32) -> String {
    use mach::cputype::*;
    // the top byte of the subtype holds capability flags (e.g. pointer auth)
    let sub = sub & 0x00FF_FFFF;
    match cputype {
        CPU_TYPE_ARM64 => if sub == 2 { "arm64e" } else { "arm64" }.into(),
        CPU_TYPE_ARM64_32 => "arm64_32".into(),
        CPU_TYPE_X86_64 => if sub == 8 { "x86_64h" } else { "x86_64" }.into(),
        CPU_TYPE_X86 => "i386".into(),
        _ => get_arch_name_from_types(cputype, sub)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("cpu {cputype:#x}")),
    }
}

fn parse_macho(m: &mach::MachO, base_off: u64) -> FormatInfo {
    let h = &m.header;
    let mut info = FormatInfo {
        kind: "Mach-O".into(),
        arch: mach_arch(h.cputype),
        ..Default::default()
    };
    let ft = mach::header::filetype_to_str(h.filetype);
    info.description = format!(
        "Mach-O {} {ft}, {}",
        if m.is_64 { "64-bit" } else { "32-bit" },
        mach_arch_name(h.cputype, h.cpusubtype)
    );
    let hsize = if m.is_64 { 32 } else { 28 } + h.sizeofcmds as u64;
    info.regions.push(Region {
        name: "Mach-O header + load commands".into(),
        kind: "header",
        offset: base_off,
        size: hsize,
        vaddr: None,
        flags: None,
    });
    info.tree.push(
        Node {
            children: vec![
                Node::field("CPU", mach_arch_name(h.cputype, h.cpusubtype)),
                Node::field("File type", ft),
                Node::field("Load commands", h.ncmds.to_string()),
                Node::field("Flags", hex(h.flags as u64)),
            ],
            ..Node::group("Mach header", vec![])
        }
        .at(base_off, hsize),
    );

    let mut segs = vec![];
    for seg in m.segments.iter() {
        let name = seg.name().unwrap_or("?").to_string();
        if seg.vmsize > 0 {
            info.mappings.push(Mapping {
                vaddr: seg.vmaddr,
                vsize: seg.vmsize,
                offset: base_off + seg.fileoff,
                fsize: seg.filesize,
            });
        }
        let prot = seg.initprot;
        let flags = format!(
            "{}{}{}",
            if prot & 1 != 0 { 'r' } else { '-' },
            if prot & 2 != 0 { 'w' } else { '-' },
            if prot & 4 != 0 { 'x' } else { '-' }
        );
        let mut children = vec![];
        if let Ok(sections) = seg.sections() {
            for (s, _) in sections {
                let sname = format!("{},{}", s.segname().unwrap_or("?"), s.name().unwrap_or("?"));
                let zerofill = s.flags & 0xFF == mach::constants::S_ZEROFILL
                    || s.flags & 0xFF == mach::constants::S_GB_ZEROFILL
                    || s.flags & 0xFF == mach::constants::S_THREAD_LOCAL_ZEROFILL;
                let mut n = Node {
                    label: sname.clone(),
                    value: Some(format!(
                        "off {} size {} va {}",
                        hex(s.offset as u64),
                        hex(s.size),
                        hex(s.addr)
                    )),
                    ..Default::default()
                };
                if !zerofill && s.size > 0 && s.offset > 0 {
                    n = n.at(base_off + s.offset as u64, s.size);
                    info.regions.push(Region {
                        name: sname,
                        kind: "section",
                        offset: base_off + s.offset as u64,
                        size: s.size,
                        vaddr: Some(s.addr),
                        flags: None,
                    });
                }
                children.push(n);
            }
        }
        let mut n = Node {
            label: name.clone(),
            value: Some(format!(
                "{flags} off {} size {} va {}",
                hex(seg.fileoff),
                hex(seg.filesize),
                hex(seg.vmaddr)
            )),
            children,
            ..Default::default()
        };
        if seg.filesize > 0 {
            n = n.at(base_off + seg.fileoff, seg.filesize);
            info.regions.push(Region {
                name,
                kind: "segment",
                offset: base_off + seg.fileoff,
                size: seg.filesize,
                vaddr: Some(seg.vmaddr),
                flags: Some(flags),
            });
        }
        segs.push(n);
    }
    info.tree.push(Node::group("Segments", segs));

    if m.entry != 0 {
        info.entry = Some(m.entry);
        info.entry_offset = va_to_offset(&info.mappings, m.entry);
    }

    if !m.libs.is_empty() {
        info.tree.push(Node::group(
            "Libraries",
            m.libs
                .iter()
                .filter(|l| **l != "self")
                .map(|l| Node::field(*l, ""))
                .collect(),
        ));
    }
    if let Ok(imports) = m.imports() {
        let nodes: Vec<Node> = imports
            .iter()
            .take(MAX_SYMBOLS)
            .map(|i| Node::field(i.name, i.dylib))
            .collect();
        if !nodes.is_empty() {
            info.tree.push(Node::group("Imports", nodes));
        }
    }
    for (name, nl) in m.symbols().flatten() {
        if info.symbols.len() >= MAX_SYMBOLS {
            break;
        }
        if name.is_empty() || nl.n_value == 0 || nl.is_undefined() {
            continue;
        }
        info.symbols.push(Symbol {
            name: name.to_string(),
            vaddr: nl.n_value,
            offset: va_to_offset(&info.mappings, nl.n_value),
            size: 0,
            kind: if nl.is_stab() { "debug" } else { "symbol" },
        });
    }
    info
}

fn parse_fat(f: &mach::MultiArch) -> FormatInfo {
    let mut info = FormatInfo {
        kind: "Mach-O (universal)".into(),
        ..Default::default()
    };
    let arches: Vec<_> = f.iter_arches().flatten().collect();
    let names: Vec<String> = arches
        .iter()
        .map(|a| mach_arch_name(a.cputype, a.cpusubtype))
        .collect();
    info.description = format!("Mach-O universal binary: {}", names.join(", "));
    info.regions.push(Region {
        name: "Fat header".into(),
        kind: "header",
        offset: 0,
        size: 8 + 20 * arches.len() as u64,
        vaddr: None,
        flags: None,
    });
    // Prefer the slice that matches the host for details.
    let preferred = arches
        .iter()
        .position(|a| a.cputype == mach::cputype::CPU_TYPE_ARM64)
        .or_else(|| {
            arches
                .iter()
                .position(|a| a.cputype == mach::cputype::CPU_TYPE_X86_64)
        })
        .unwrap_or(0);
    let mut slices = vec![];
    for (i, a) in arches.iter().enumerate() {
        let mut node = Node {
            label: names[i].clone(),
            value: Some(format!(
                "off {} size {}",
                hex(a.offset as u64),
                hex(a.size as u64)
            )),
            ..Default::default()
        }
        .at(a.offset as u64, a.size as u64);
        info.regions.push(Region {
            name: format!("slice {}", names[i]),
            kind: "slice",
            offset: a.offset as u64,
            size: a.size as u64,
            vaddr: None,
            flags: None,
        });
        if i == preferred {
            if let Ok(mach::SingleArch::MachO(m)) = f.get(i) {
                let inner = parse_macho(&m, a.offset as u64);
                info.arch = inner.arch;
                info.entry = inner.entry;
                info.entry_offset = inner.entry_offset;
                info.mappings = inner.mappings;
                info.symbols = inner.symbols;
                info.regions.extend(inner.regions);
                node.children = inner.tree;
                node.value = Some(format!("{} (shown)", node.value.unwrap_or_default()));
            }
        }
        slices.push(node);
    }
    info.tree.push(Node::group("Architectures", slices));
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_magic() {
        assert_eq!(analyze(b"\x89PNG\r\n\x1a\n....").kind, "PNG");
        assert_eq!(analyze(b"hello world\n").kind, "Text");
        assert_eq!(analyze(&[0u8, 1, 2, 3, 200]).kind, "Data");
    }

    #[test]
    fn parses_own_test_binary() {
        // The test binary itself is a native executable on every platform.
        let exe = std::env::current_exe().unwrap();
        let data = std::fs::read(exe).unwrap();
        let info = analyze(&data);
        assert!(
            ["ELF", "PE", "Mach-O", "Mach-O (universal)"].contains(&info.kind.as_str()),
            "{}",
            info.kind
        );
        assert!(info.arch.is_some());
        assert!(!info.regions.is_empty());
        assert!(!info.mappings.is_empty());
        let entry = info.entry_offset.expect("entry offset");
        assert!(entry < data.len() as u64);
        assert_eq!(info.va_to_offset(info.entry.unwrap() & !1), Some(entry));
        assert_eq!(info.offset_to_va(entry), Some(info.entry.unwrap() & !1));
    }
}
