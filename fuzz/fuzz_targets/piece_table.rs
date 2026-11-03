#![no_main]
//! Random edit sequences must keep the piece table identical to a plain Vec.

use arbitrary::Arbitrary;
use hexly_core::buffer::{PieceTable, Source};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(u16, Vec<u8>),
    Delete(u16, u8),
    Overwrite(u16, Vec<u8>),
    Undo,
    Redo,
}

fuzz_target!(|input: (Vec<u8>, Vec<Op>)| {
    let (init, ops) = input;
    let mut t = PieceTable::new(Source::Mem(init.clone()));
    let mut model = init;
    let mut undo: Vec<Vec<u8>> = vec![];
    let mut redo: Vec<Vec<u8>> = vec![];
    for op in ops.into_iter().take(200) {
        let len = model.len();
        match op {
            Op::Insert(o, d) if !d.is_empty() => {
                let o = o as usize % (len + 1);
                undo.push(model.clone());
                redo.clear();
                model.splice(o..o, d.iter().copied());
                t.insert(o as u64, &d);
            }
            Op::Delete(o, l) if len > 0 && l > 0 => {
                let o = o as usize % len;
                let end = (o + l as usize).min(len);
                undo.push(model.clone());
                redo.clear();
                model.drain(o..end);
                t.delete(o as u64, l as u64);
            }
            Op::Overwrite(o, d) if !d.is_empty() => {
                let o = o as usize % (len + 1);
                undo.push(model.clone());
                redo.clear();
                let n = (len - o).min(d.len());
                model.splice(o..o + n, d.iter().copied());
                t.overwrite(o as u64, &d);
            }
            Op::Undo => {
                if let Some(p) = undo.pop() {
                    redo.push(std::mem::replace(&mut model, p));
                }
                t.undo();
            }
            Op::Redo => {
                if let Some(n) = redo.pop() {
                    undo.push(std::mem::replace(&mut model, n));
                }
                t.redo();
            }
            _ => {}
        }
        assert_eq!(t.read_vec(0, t.len() as usize), model);
    }
});
