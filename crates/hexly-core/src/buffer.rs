//! Piece table over an immutable original source plus an append-only add buffer.
//!
//! The original file is never modified while editing. Every edit only rearranges
//! the list of pieces, which makes edits on multi-gigabyte files cheap and lets
//! undo/redo store just the slice of the piece list that changed.

use memmap2::Mmap;

/// The immutable bytes a document was opened from.
pub enum Source {
    Mem(Vec<u8>),
    Map(Mmap),
}

impl Source {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Source::Mem(v) => v,
            Source::Map(m) => m,
        }
    }
}

impl Default for Source {
    fn default() -> Self {
        Source::Mem(Vec::new())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Original,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Piece {
    kind: Kind,
    start: u64,
    len: u64,
}

/// One undoable change: `removed` pieces at `index` were replaced by `inserted`.
#[derive(Clone, Debug)]
struct Change {
    index: usize,
    removed: Vec<Piece>,
    inserted: Vec<Piece>,
    before_id: u64,
    after_id: u64,
    /// Offset the cursor should go to when this change is undone/redone.
    offset: u64,
}

#[derive(Default)]
pub struct PieceTable {
    original: Source,
    add: Vec<u8>,
    pieces: Vec<Piece>,
    /// starts[i] = document offset of pieces[i]
    starts: Vec<u64>,
    len: u64,
    undo: Vec<Change>,
    redo: Vec<Change>,
    /// Id of the current content state; changes on every edit/undo/redo.
    state_id: u64,
    next_id: u64,
    /// Pieces captured when the current transaction started.
    tx: Option<(Vec<Piece>, u64)>,
}

/// Maximum number of undo steps kept.
const MAX_UNDO: usize = 100_000;

impl PieceTable {
    pub fn new(original: Source) -> Self {
        let len = original.bytes().len() as u64;
        let mut t = PieceTable {
            original,
            next_id: 1,
            ..Default::default()
        };
        if len > 0 {
            t.pieces.push(Piece {
                kind: Kind::Original,
                start: 0,
                len,
            });
        }
        t.reindex();
        t
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn state_id(&self) -> u64 {
        self.state_id
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// True when the content is exactly the unmodified original source.
    pub fn is_pristine(&self) -> bool {
        self.pieces.is_empty() && self.original.bytes().is_empty()
            || (self.pieces.len() == 1
                && self.pieces[0].kind == Kind::Original
                && self.pieces[0].start == 0
                && self.pieces[0].len == self.original.bytes().len() as u64)
    }

    /// The original bytes, if the document is unmodified (zero-copy access).
    pub fn pristine_bytes(&self) -> Option<&[u8]> {
        if self.is_pristine() {
            Some(self.original.bytes())
        } else {
            None
        }
    }

    fn reindex(&mut self) {
        self.starts.clear();
        self.starts.reserve(self.pieces.len());
        let mut acc = 0u64;
        for p in &self.pieces {
            self.starts.push(acc);
            acc += p.len;
        }
        self.len = acc;
    }

    fn piece_bytes(&self, p: &Piece) -> &[u8] {
        let src = match p.kind {
            Kind::Original => self.original.bytes(),
            Kind::Add => &self.add,
        };
        &src[p.start as usize..(p.start + p.len) as usize]
    }

    /// Index of the piece containing `offset` (offset < len).
    fn locate(&self, offset: u64) -> usize {
        self.starts.partition_point(|&s| s <= offset) - 1
    }

    /// Reads up to `buf.len()` bytes at `offset`. Returns the number read.
    pub fn read(&self, offset: u64, buf: &mut [u8]) -> usize {
        if offset >= self.len || buf.is_empty() {
            return 0;
        }
        let want = ((self.len - offset) as usize).min(buf.len());
        let mut i = self.locate(offset);
        let mut in_piece = offset - self.starts[i];
        let mut done = 0;
        while done < want {
            let bytes = self.piece_bytes(&self.pieces[i]);
            let avail = &bytes[in_piece as usize..];
            let n = avail.len().min(want - done);
            buf[done..done + n].copy_from_slice(&avail[..n]);
            done += n;
            i += 1;
            in_piece = 0;
        }
        done
    }

    pub fn read_vec(&self, offset: u64, len: usize) -> Vec<u8> {
        let avail = self.len.saturating_sub(offset).min(len as u64) as usize;
        let mut v = vec![0; avail];
        let n = self.read(offset, &mut v);
        v.truncate(n);
        v
    }

    pub fn byte_at(&self, offset: u64) -> Option<u8> {
        let mut b = [0u8];
        (self.read(offset, &mut b) == 1).then_some(b[0])
    }

    /// Calls `f` with consecutive contiguous slices covering `[offset, offset+len)`.
    pub fn for_each_slice(&self, offset: u64, len: u64, mut f: impl FnMut(u64, &[u8]) -> bool) {
        if offset >= self.len {
            return;
        }
        let end = (offset + len).min(self.len);
        let mut i = self.locate(offset);
        let mut pos = offset;
        while pos < end && i < self.pieces.len() {
            let p = self.pieces[i];
            let ps = self.starts[i];
            let bytes = self.piece_bytes(&p);
            let from = (pos - ps) as usize;
            let to = ((end - ps).min(p.len)) as usize;
            if !f(pos, &bytes[from..to]) {
                return;
            }
            pos = ps + to as u64;
            i += 1;
        }
    }

    /// Ensures a piece boundary at `offset` and returns the index of the piece starting there.
    fn split_at(&mut self, offset: u64) -> usize {
        if offset >= self.len {
            return self.pieces.len();
        }
        let i = self.locate(offset);
        let ps = self.starts[i];
        if ps == offset {
            return i;
        }
        let p = self.pieces[i];
        let left = offset - ps;
        self.pieces[i].len = left;
        self.pieces.insert(
            i + 1,
            Piece {
                kind: p.kind,
                start: p.start + left,
                len: p.len - left,
            },
        );
        self.starts.insert(i + 1, offset);
        i + 1
    }

    fn normalize(&mut self) {
        let mut out: Vec<Piece> = Vec::with_capacity(self.pieces.len());
        for p in self.pieces.drain(..) {
            if p.len == 0 {
                continue;
            }
            if let Some(last) = out.last_mut() {
                if last.kind == p.kind && last.start + last.len == p.start {
                    last.len += p.len;
                    continue;
                }
            }
            out.push(p);
        }
        self.pieces = out;
        self.reindex();
    }

    // ---- transactions -------------------------------------------------

    /// Starts grouping edits into one undo step. Nested calls are ignored.
    pub fn begin(&mut self, offset: u64) {
        if self.tx.is_none() {
            self.tx = Some((self.pieces.clone(), offset));
        }
    }

    /// Ends the current transaction and records it for undo if anything changed.
    pub fn commit(&mut self) -> bool {
        let Some((old, offset)) = self.tx.take() else {
            return false;
        };
        self.normalize();
        let new = &self.pieces;
        let prefix = old
            .iter()
            .zip(new.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let max_suffix = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(max_suffix)
            .take_while(|(a, b)| a == b)
            .count();
        let removed = old[prefix..old.len() - suffix].to_vec();
        let inserted = new[prefix..new.len() - suffix].to_vec();
        if removed.is_empty() && inserted.is_empty() {
            return false;
        }
        let before_id = self.state_id;
        self.state_id = self.next_id;
        self.next_id += 1;
        self.undo.push(Change {
            index: prefix,
            removed,
            inserted,
            before_id,
            after_id: self.state_id,
            offset,
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    fn with_tx<R>(&mut self, offset: u64, f: impl FnOnce(&mut Self) -> R) -> R {
        let own = self.tx.is_none();
        if own {
            self.begin(offset);
        }
        let r = f(self);
        if own {
            self.commit();
        } else {
            self.reindex();
        }
        r
    }

    // ---- edits ----------------------------------------------------------

    pub fn insert(&mut self, offset: u64, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let offset = offset.min(self.len);
        self.with_tx(offset, |t| {
            let start = t.add.len() as u64;
            t.add.extend_from_slice(data);
            let i = t.split_at(offset);
            t.pieces.insert(
                i,
                Piece {
                    kind: Kind::Add,
                    start,
                    len: data.len() as u64,
                },
            );
            t.reindex();
        });
    }

    pub fn delete(&mut self, offset: u64, len: u64) {
        if offset >= self.len || len == 0 {
            return;
        }
        let end = (offset + len).min(self.len);
        self.with_tx(offset, |t| {
            let i = t.split_at(offset);
            let j = t.split_at(end);
            t.pieces.drain(i..j);
            t.reindex();
        });
    }

    /// Overwrites bytes at `offset`; writing past the end extends the document.
    pub fn overwrite(&mut self, offset: u64, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let offset = offset.min(self.len);
        self.with_tx(offset, |t| {
            let n = (t.len - offset).min(data.len() as u64);
            t.delete(offset, n);
            t.insert(offset, data);
        });
    }

    /// Replaces `len` bytes at `offset` with `data` (sizes may differ).
    pub fn replace(&mut self, offset: u64, len: u64, data: &[u8]) {
        self.with_tx(offset, |t| {
            t.delete(offset, len);
            t.insert(offset, data);
        });
    }

    /// Undoes the last change; returns the offset it touched.
    pub fn undo(&mut self) -> Option<u64> {
        let c = self.undo.pop()?;
        self.pieces.splice(
            c.index..c.index + c.inserted.len(),
            c.removed.iter().copied(),
        );
        self.reindex();
        self.state_id = c.before_id;
        let off = c.offset;
        self.redo.push(c);
        Some(off)
    }

    pub fn redo(&mut self) -> Option<u64> {
        let c = self.redo.pop()?;
        self.pieces.splice(
            c.index..c.index + c.removed.len(),
            c.inserted.iter().copied(),
        );
        self.reindex();
        self.state_id = c.after_id;
        let off = c.offset;
        self.undo.push(c);
        Some(off)
    }

    /// Writes the whole content to `w`.
    pub fn write_to(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        for p in &self.pieces {
            w.write_all(self.piece_bytes(p))?;
        }
        Ok(())
    }

    /// Contiguous view of the whole content, borrowing when possible.
    pub fn contiguous(&self) -> std::borrow::Cow<'_, [u8]> {
        match self.pristine_bytes() {
            Some(b) => std::borrow::Cow::Borrowed(b),
            None => std::borrow::Cow::Owned(self.read_vec(0, self.len as usize)),
        }
    }

    /// Number of pieces (for diagnostics/tests).
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn table(data: &[u8]) -> PieceTable {
        PieceTable::new(Source::Mem(data.to_vec()))
    }

    fn all(t: &PieceTable) -> Vec<u8> {
        t.read_vec(0, t.len() as usize)
    }

    #[test]
    fn basic_edits() {
        let mut t = table(b"hello world");
        t.overwrite(0, b"J");
        assert_eq!(all(&t), b"Jello world");
        t.insert(5, b",");
        assert_eq!(all(&t), b"Jello, world");
        t.delete(0, 1);
        assert_eq!(all(&t), b"ello, world");
        t.overwrite(10, b"DXYZ");
        assert_eq!(all(&t), b"ello, worlDXYZ");
        t.undo();
        assert_eq!(all(&t), b"ello, world");
        t.undo();
        t.undo();
        t.undo();
        assert_eq!(all(&t), b"hello world");
        assert!(t.is_pristine());
        t.redo();
        assert_eq!(all(&t), b"Jello world");
    }

    #[test]
    fn sequential_typing_stays_compact() {
        let mut t = table(&[0u8; 1000]);
        for i in 0..500u64 {
            t.overwrite(100 + i, &[0xAB]);
        }
        assert_eq!(t.piece_count(), 3);
        assert_eq!(t.read_vec(99, 3), vec![0, 0xAB, 0xAB]);
    }

    #[test]
    fn transaction_is_one_undo_step() {
        let mut t = table(b"aaaa");
        t.begin(0);
        t.overwrite(0, b"b");
        t.overwrite(2, b"c");
        t.insert(4, b"dd");
        t.commit();
        assert_eq!(
            all(&t),
            b"baca".iter().chain(b"dd").copied().collect::<Vec<_>>()
        );
        t.undo();
        assert_eq!(all(&t), b"aaaa");
        assert!(!t.can_undo());
    }

    #[test]
    fn empty_document() {
        let mut t = table(b"");
        assert_eq!(t.len(), 0);
        t.insert(0, b"abc");
        assert_eq!(all(&t), b"abc");
        t.overwrite(3, b"d");
        assert_eq!(all(&t), b"abcd");
    }

    #[derive(Debug, Clone)]
    enum Op {
        Insert(u16, Vec<u8>),
        Delete(u16, u16),
        Overwrite(u16, Vec<u8>),
        Undo,
        Redo,
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (any::<u16>(), proptest::collection::vec(any::<u8>(), 1..20))
                .prop_map(|(o, d)| Op::Insert(o, d)),
            (any::<u16>(), 1u16..40).prop_map(|(o, l)| Op::Delete(o, l)),
            (any::<u16>(), proptest::collection::vec(any::<u8>(), 1..20))
                .prop_map(|(o, d)| Op::Overwrite(o, d)),
            Just(Op::Undo),
            Just(Op::Redo),
        ]
    }

    proptest! {
        /// The piece table must behave exactly like a plain Vec, including undo/redo.
        #[test]
        fn matches_vec_model(init in proptest::collection::vec(any::<u8>(), 0..300), ops in proptest::collection::vec(op(), 0..60)) {
            let mut t = table(&init);
            let mut model = init.clone();
            let mut undo: Vec<Vec<u8>> = vec![];
            let mut redo: Vec<Vec<u8>> = vec![];
            for o in ops {
                let len = model.len();
                match o {
                    Op::Insert(off, d) => {
                        let off = if len == 0 { 0 } else { off as usize % (len + 1) };
                        undo.push(model.clone()); redo.clear();
                        model.splice(off..off, d.iter().copied());
                        t.insert(off as u64, &d);
                    }
                    Op::Delete(off, l) => {
                        if len == 0 { continue; }
                        let off = off as usize % len;
                        let end = (off + l as usize).min(len);
                        undo.push(model.clone()); redo.clear();
                        model.drain(off..end);
                        t.delete(off as u64, l as u64);
                    }
                    Op::Overwrite(off, d) => {
                        let off = if len == 0 { 0 } else { off as usize % (len + 1) };
                        undo.push(model.clone()); redo.clear();
                        let n = (len - off).min(d.len());
                        model.splice(off..off + n, d.iter().copied());
                        t.overwrite(off as u64, &d);
                    }
                    Op::Undo => {
                        if let Some(prev) = undo.pop() { redo.push(std::mem::replace(&mut model, prev)); }
                        t.undo();
                    }
                    Op::Redo => {
                        if let Some(next) = redo.pop() { undo.push(std::mem::replace(&mut model, next)); }
                        t.redo();
                    }
                }
                prop_assert_eq!(t.len() as usize, model.len());
                prop_assert_eq!(all(&t), model.clone());
                prop_assert_eq!(t.can_undo(), !undo.is_empty());
            }
            // random reads
            for (off, &b) in model.iter().enumerate() {
                prop_assert_eq!(t.byte_at(off as u64), Some(b));
            }
        }
    }
}
