# Roadmap

## Done (0.1)

- Instant opening of huge files (memory mapped), piece table editing, unlimited undo/redo
- Atomic save that keeps permissions, revert, export selection, read-only mode
- Canvas hex view with byte coloring, section stripe, insert/overwrite modes
- Data inspector with editable values
- ELF, PE, Mach-O and universal binary parsing: headers, sections, segments, imports, exports, symbols
- Virtual address ↔ file offset, go-to expressions, follow pointer, entry point
- Disassembly (Capstone) for x86, ARM, ARM64, RISC-V, MIPS, PowerPC
- Search: hex with wildcards, text (UTF-8/16, Latin-1), regex, numbers, find all, replace all
- Strings, hashes, entropy minimap
- Side-by-side compare with difference navigation
- Bookmarks, copy as code, command palette, themes, self-update
- CI on Linux, macOS and Windows, release builds for all three, nightly fuzzing

## Next

- **Struct templates**: describe a structure once (a small declarative format, later scripting) and
  overlay it on the bytes with named, typed fields
- **Patch files**: export and apply IPS/BPS patches and a readable diff list
- **Alignment-aware compare** for files where bytes were inserted, not only changed
- **Data visualizer**: show a selection as an image (raw pixels), a waveform or a byte histogram
- **More formats**: DEX, WASM sections, ZIP central directory, PNG chunks
- **Process memory** (read-only first): open a running process' memory regions
- **Large selection operations** in the background: XOR/AND/OR with a key, bit shifts, checksum fix-ups
- Notarized macOS builds and signed Windows installers
- Flatpak and AUR packages
