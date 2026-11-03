#![no_main]
//! Arbitrary search patterns over arbitrary data must never panic.

use hexly_core::buffer::{PieceTable, Source};
use hexly_core::search::{Matcher, Query};
use hexly_core::Task;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (String, Vec<u8>, u16)| {
    let (pattern, data, from) = input;
    let t = PieceTable::new(Source::Mem(data));
    for q in [
        Query::Hex {
            pattern: pattern.clone(),
        },
        Query::Regex {
            pattern,
            case_sensitive: false,
        },
    ] {
        if let Ok(m) = Matcher::new(&q) {
            let task = Task::none();
            let _ = m.find_next(&t, from as u64, true, &task);
            let _ = m.find_prev(&t, from as u64, true, &task);
            let _ = m.find_all(&t, 100, &task);
        }
    }
});
