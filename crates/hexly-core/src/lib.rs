//! Hexly core: everything the editor does that is not UI.
//!
//! The crate has no dependency on Tauri so it can be tested, fuzzed and reused
//! on its own.

pub mod analysis;
pub mod buffer;
pub mod disasm;
pub mod document;
pub mod formats;
pub mod inspector;
pub mod search;
pub mod util;

pub use buffer::PieceTable;
pub use document::Document;

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Parse(String),
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// Cancellation and progress reporting for long-running operations.
pub struct Task<'a> {
    cancel: Option<&'a AtomicBool>,
    progress: Option<&'a (dyn Fn(f64) + Sync)>,
}

impl<'a> Task<'a> {
    pub fn none() -> Task<'static> {
        Task {
            cancel: None,
            progress: None,
        }
    }

    pub fn new(cancel: &'a AtomicBool, progress: &'a (dyn Fn(f64) + Sync)) -> Self {
        Task {
            cancel: Some(cancel),
            progress: Some(progress),
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c.load(Ordering::Relaxed))
    }

    pub fn report(&self, fraction: f64) {
        if let Some(p) = self.progress {
            p(fraction.clamp(0.0, 1.0));
        }
    }
}
