use hexly_core::formats::{self, FormatInfo};
use hexly_core::Document;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Modified documents larger than this are not re-parsed (it needs a full copy).
const MAX_REPARSE: u64 = 1 << 30;

pub struct Doc {
    pub doc: RwLock<Document>,
    /// Opened read-only by the user (independent of file permissions).
    pub locked_readonly: AtomicBool,
    format: Mutex<Option<(u64, Arc<FormatInfo>)>>,
}

impl Doc {
    /// Parses the format, cached per content state.
    pub fn format(&self) -> Arc<FormatInfo> {
        let g = self.doc.read();
        let sid = g.table.state_id();
        if let Some((s, f)) = &*self.format.lock() {
            if *s == sid {
                return f.clone();
            }
        }
        let info = if g.table.is_pristine() || g.len() <= MAX_REPARSE {
            Arc::new(formats::analyze(&g.table.contiguous()))
        } else {
            // keep the last result for very large modified files
            match &*self.format.lock() {
                Some((_, f)) => return f.clone(),
                None => Arc::new(FormatInfo::default()),
            }
        };
        *self.format.lock() = Some((sid, info.clone()));
        info
    }

    /// Last parsed format without re-parsing.
    pub fn format_cached(&self) -> Option<Arc<FormatInfo>> {
        self.format.lock().as_ref().map(|(_, f)| f.clone())
    }
}

#[derive(Default)]
pub struct AppState {
    pub docs: RwLock<HashMap<u32, Arc<Doc>>>,
    next_id: AtomicU32,
    pub tasks: Mutex<HashMap<u32, Arc<AtomicBool>>>,
    /// Files passed on the command line or via the OS before the UI asked for them.
    pub pending_open: Mutex<Vec<String>>,
}

impl AppState {
    pub fn insert(&self, doc: Document, readonly: bool) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.docs.write().insert(
            id,
            Arc::new(Doc {
                doc: RwLock::new(doc),
                locked_readonly: AtomicBool::new(readonly),
                format: Mutex::new(None),
            }),
        );
        id
    }

    pub fn doc(&self, id: u32) -> Option<Arc<Doc>> {
        self.docs.read().get(&id).cloned()
    }
}
