//! A document: a piece table bound to an (optional) file on disk.

use crate::buffer::{PieceTable, Source};
use crate::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Files up to this size are read into memory, larger ones are memory mapped.
const MMAP_THRESHOLD: u64 = 128 * 1024 * 1024;

pub struct Document {
    pub table: PieceTable,
    path: Option<PathBuf>,
    readonly: bool,
    saved_state: u64,
    disk_stamp: Option<(Option<SystemTime>, u64)>,
}

fn stamp(path: &Path) -> Option<(Option<SystemTime>, u64)> {
    let m = fs::metadata(path).ok()?;
    Some((m.modified().ok(), m.len()))
}

fn load_source(path: &Path) -> Result<Source, Error> {
    let file = File::open(path)?;
    let meta = file.metadata()?;
    if meta.is_dir() {
        return Err(Error::Other(format!("{} is a directory", path.display())));
    }
    let len = meta.len();
    if len == 0 {
        return Ok(Source::Mem(Vec::new()));
    }
    if len <= MMAP_THRESHOLD || !meta.is_file() {
        let mut v = Vec::with_capacity(len as usize);
        std::io::Read::read_to_end(&mut &file, &mut v)?;
        return Ok(Source::Mem(v));
    }
    // SAFETY: the mapping is read-only. If another process truncates the file
    // while it is open, access may fault; this is the usual trade-off every
    // mmap-based editor makes in exchange for instant opening of huge files.
    let map = unsafe { memmap2::Mmap::map(&file)? };
    Ok(Source::Map(map))
}

impl Document {
    pub fn new_empty() -> Self {
        Document {
            table: PieceTable::new(Source::default()),
            path: None,
            readonly: false,
            saved_state: 0,
            disk_stamp: None,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Document {
            table: PieceTable::new(Source::Mem(bytes)),
            ..Self::new_empty()
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = dunce::canonicalize(path.as_ref()).unwrap_or_else(|_| path.as_ref().to_path_buf());
        let source = load_source(&path)?;
        let readonly = OpenOptions::new().append(true).open(&path).is_err();
        Ok(Document {
            table: PieceTable::new(source),
            disk_stamp: stamp(&path),
            path: Some(path),
            readonly,
            saved_state: 0,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn readonly(&self) -> bool {
        self.readonly
    }

    pub fn len(&self) -> u64 {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub fn modified(&self) -> bool {
        self.table.state_id() != self.saved_state
    }

    /// True when the file on disk changed since it was opened or saved.
    pub fn changed_on_disk(&self) -> bool {
        match (&self.path, &self.disk_stamp) {
            (Some(p), Some(s)) => stamp(p).as_ref() != Some(s),
            _ => false,
        }
    }

    /// Saves to the current path.
    pub fn save(&mut self) -> Result<(), Error> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| Error::Other("document has no file path yet".into()))?;
        if self.readonly {
            return Err(Error::Other("file is read-only".into()));
        }
        self.save_to(&path)
    }

    /// Writes the document to `target` atomically (temp file + rename) and
    /// rebinds the document to it. Undo history is cleared because it refers
    /// to the old file contents.
    pub fn save_as(&mut self, target: impl AsRef<Path>) -> Result<(), Error> {
        self.save_to(target.as_ref())
    }

    fn save_to(&mut self, target: &Path) -> Result<(), Error> {
        // Follow symlinks so we replace the real file, not the link.
        let target = dunce::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
        let dir = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        let tmp = dir.join(format!(".{name}.hexly-{}.tmp", std::process::id()));

        let write = || -> Result<(), Error> {
            let f = File::create(&tmp)?;
            let mut w = BufWriter::with_capacity(4 << 20, f);
            self.table.write_to(&mut w)?;
            let f = w.into_inner().map_err(|e| e.into_error())?;
            f.sync_all()?;
            // Keep permissions (e.g. the executable bit) of the file we replace.
            if let Ok(meta) = fs::metadata(&target) {
                fs::set_permissions(&tmp, meta.permissions())?;
            }
            Ok(())
        };
        if let Err(e) = write() {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }

        // Release the mapping before replacing the file (required on Windows).
        let same_file = self.path.as_deref() == Some(target.as_path());
        if same_file {
            self.table = PieceTable::new(Source::default());
        }
        if let Err(e) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            if same_file {
                // Reload the untouched original; unsaved edits are lost only
                // if this fails too, which is reported to the caller.
                self.table = PieceTable::new(load_source(&target)?);
                self.saved_state = self.table.state_id();
                return Err(Error::Other(format!(
                    "could not replace {}: {e}. The file on disk is unchanged, but unsaved edits were discarded.",
                    target.display()
                )));
            }
            return Err(e.into());
        }
        self.table = PieceTable::new(load_source(&target)?);
        self.saved_state = self.table.state_id();
        self.disk_stamp = stamp(&target);
        self.readonly = false;
        self.path = Some(dunce::canonicalize(&target).unwrap_or(target));
        Ok(())
    }

    /// Writes a byte range to a new file (export selection).
    pub fn export_range(
        &self,
        offset: u64,
        len: u64,
        target: impl AsRef<Path>,
    ) -> Result<(), Error> {
        let f = File::create(target)?;
        let mut w = BufWriter::new(f);
        let mut err = None;
        self.table
            .for_each_slice(offset, len, |_, s| match w.write_all(s) {
                Ok(()) => true,
                Err(e) => {
                    err = Some(e);
                    false
                }
            });
        if let Some(e) = err {
            return Err(e.into());
        }
        w.flush()?;
        Ok(())
    }

    /// Reloads the file from disk, discarding edits.
    pub fn reload(&mut self) -> Result<(), Error> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| Error::Other("document has no file path".into()))?;
        self.table = PieceTable::new(Source::default());
        self.table = PieceTable::new(load_source(&path)?);
        self.saved_state = self.table.state_id();
        self.disk_stamp = stamp(&path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_roundtrip_keeps_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bin");
        fs::write(&p, b"\x7fELF....").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut d = Document::open(&p).unwrap();
        assert!(!d.modified());
        d.table.overwrite(4, b"ABCD");
        d.table.insert(8, b"!");
        assert!(d.modified());
        d.save().unwrap();
        assert!(!d.modified());
        assert_eq!(fs::read(&p).unwrap(), b"\x7fELFABCD!");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        // no temp files left behind
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn undo_back_to_saved_is_unmodified() {
        let mut d = Document::from_bytes(b"abc".to_vec());
        d.table.overwrite(0, b"x");
        assert!(d.modified());
        d.table.undo();
        assert!(!d.modified());
    }

    #[test]
    fn save_as_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Document::new_empty();
        d.table.insert(0, b"hello");
        let p = dir.path().join("out.bin");
        d.save_as(&p).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"hello");
        assert_eq!(d.path().unwrap(), dunce::canonicalize(&p).unwrap());
    }
}
