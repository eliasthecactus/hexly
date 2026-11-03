//! Tauri commands: a thin layer over `hexly-core`.

use crate::state::{AppState, Doc};
use hexly_core::analysis::{self, EntropyMap, FoundString, HashResult, StringsOptions};
use hexly_core::disasm::{self, Arch, Insn};
use hexly_core::formats::FormatInfo;
use hexly_core::inspector::{self, Field, NumType};
use hexly_core::search::{self, Match, Matcher, Query};
use hexly_core::util::{self, CopyFormat};
use hexly_core::{Document, Task};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocInfo {
    id: u32,
    path: Option<String>,
    name: String,
    size: u64,
    readonly: bool,
    modified: bool,
    can_undo: bool,
    can_redo: bool,
    state_id: u64,
}

fn info(id: u32, d: &Document) -> DocInfo {
    DocInfo {
        id,
        path: d.path().map(|p| p.to_string_lossy().into_owned()),
        name: d
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into()),
        size: d.len(),
        readonly: d.readonly(),
        modified: d.modified(),
        can_undo: d.table.can_undo(),
        can_redo: d.table.can_redo(),
        state_id: d.table.state_id(),
    }
}

fn get(state: &AppState, id: u32) -> R<Arc<Doc>> {
    state
        .doc(id)
        .ok_or_else(|| "document is not open".to_string())
}

fn writable(doc: &Doc) -> R<()> {
    if doc.locked_readonly.load(Ordering::Relaxed) {
        return Err("document is opened read-only".into());
    }
    Ok(())
}

/// Runs blocking work with cancellation and throttled progress events.
async fn run_task<T: Send + 'static>(
    app: AppHandle,
    state: &AppState,
    task_id: u32,
    f: impl FnOnce(&Task) -> T + Send + 'static,
) -> R<T> {
    let cancel = Arc::new(AtomicBool::new(false));
    state.tasks.lock().insert(task_id, cancel.clone());
    let res = tauri::async_runtime::spawn_blocking(move || {
        let last = AtomicU32::new(0);
        let progress = |p: f64| {
            let pct = (p * 100.0) as u32;
            if pct != last.swap(pct, Ordering::Relaxed) {
                let _ = app.emit("task-progress", (task_id, p));
            }
        };
        let task = Task::new(&cancel, &progress);
        f(&task)
    })
    .await
    .map_err(err);
    state.tasks.lock().remove(&task_id);
    res
}

#[tauri::command]
pub fn cancel_task(state: State<AppState>, task_id: u32) {
    if let Some(c) = state.tasks.lock().get(&task_id) {
        c.store(true, Ordering::Relaxed);
    }
}

// ---- documents ------------------------------------------------------------------

#[tauri::command]
pub async fn open_file(
    state: State<'_, AppState>,
    path: String,
    readonly: Option<bool>,
) -> R<DocInfo> {
    let p = PathBuf::from(&path);
    let doc = tauri::async_runtime::spawn_blocking(move || Document::open(p))
        .await
        .map_err(err)?
        .map_err(err)?;
    let id = state.insert(doc, readonly.unwrap_or(false));
    let d = get(&state, id)?;
    let g = d.doc.read();
    let mut i = info(id, &g);
    i.readonly |= readonly.unwrap_or(false);
    Ok(i)
}

#[tauri::command]
pub fn new_document(state: State<AppState>) -> DocInfo {
    let id = state.insert(Document::new_empty(), false);
    info(id, &get(&state, id).unwrap().doc.read())
}

#[tauri::command]
pub fn close_document(state: State<AppState>, doc: u32) {
    state.docs.write().remove(&doc);
}

#[tauri::command]
pub fn doc_info(state: State<AppState>, doc: u32) -> R<DocInfo> {
    let d = get(&state, doc)?;
    let mut i = info(doc, &d.doc.read());
    i.readonly |= d.locked_readonly.load(Ordering::Relaxed);
    Ok(i)
}

#[tauri::command]
pub fn take_pending_opens(state: State<AppState>) -> Vec<String> {
    std::mem::take(&mut *state.pending_open.lock())
}

#[tauri::command]
pub fn read_range(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    len: u32,
) -> R<tauri::ipc::Response> {
    let d = get(&state, doc)?;
    let len = len.min(16 << 20) as usize;
    let bytes = d.doc.read().table.read_vec(offset, len);
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub fn changed_on_disk(state: State<AppState>, doc: u32) -> R<bool> {
    Ok(get(&state, doc)?.doc.read().changed_on_disk())
}

#[tauri::command]
pub async fn save(state: State<'_, AppState>, doc: u32) -> R<DocInfo> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let d2 = d.clone();
    tauri::async_runtime::spawn_blocking(move || d2.doc.write().save())
        .await
        .map_err(err)?
        .map_err(err)?;
    let i = info(doc, &d.doc.read());
    Ok(i)
}

#[tauri::command]
pub async fn save_as(state: State<'_, AppState>, doc: u32, path: String) -> R<DocInfo> {
    let d = get(&state, doc)?;
    let d2 = d.clone();
    tauri::async_runtime::spawn_blocking(move || d2.doc.write().save_as(path))
        .await
        .map_err(err)?
        .map_err(err)?;
    d.locked_readonly.store(false, Ordering::Relaxed);
    let i = info(doc, &d.doc.read());
    Ok(i)
}

#[tauri::command]
pub async fn export_range(
    state: State<'_, AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    path: String,
) -> R<()> {
    let d = get(&state, doc)?;
    tauri::async_runtime::spawn_blocking(move || d.doc.read().export_range(offset, len, path))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub fn reload(state: State<AppState>, doc: u32) -> R<DocInfo> {
    let d = get(&state, doc)?;
    let mut g = d.doc.write();
    g.reload().map_err(err)?;
    Ok(info(doc, &g))
}

// ---- editing --------------------------------------------------------------------

#[tauri::command]
pub fn write_bytes(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    data: Vec<u8>,
    insert: bool,
) -> R<DocInfo> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let mut g = d.doc.write();
    if insert {
        g.table.insert(offset, &data);
    } else {
        g.table.overwrite(offset, &data);
    }
    Ok(info(doc, &g))
}

#[tauri::command]
pub fn replace_range(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    data: Vec<u8>,
) -> R<DocInfo> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let mut g = d.doc.write();
    g.table.replace(offset, len, &data);
    Ok(info(doc, &g))
}

#[tauri::command]
pub fn delete_range(state: State<AppState>, doc: u32, offset: u64, len: u64) -> R<DocInfo> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let mut g = d.doc.write();
    g.table.delete(offset, len);
    Ok(info(doc, &g))
}

/// Fills `[offset, offset+len)` with a repeating hex pattern.
#[tauri::command]
pub fn fill_range(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    pattern: String,
    insert: bool,
) -> R<DocInfo> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let pat = search::parse_hex_bytes(&pattern).map_err(err)?;
    if pat.is_empty() {
        return Err("pattern is empty".into());
    }
    if len > 1 << 30 {
        return Err("fill is limited to 1 GiB".into());
    }
    let data: Vec<u8> = pat.iter().copied().cycle().take(len as usize).collect();
    let mut g = d.doc.write();
    if insert {
        g.table.insert(offset, &data);
    } else {
        g.table.overwrite(offset, &data);
    }
    Ok(info(doc, &g))
}

#[derive(Serialize)]
pub struct UndoResult {
    info: DocInfo,
    offset: Option<u64>,
}

#[tauri::command]
pub fn undo(state: State<AppState>, doc: u32) -> R<UndoResult> {
    let d = get(&state, doc)?;
    let mut g = d.doc.write();
    let offset = g.table.undo();
    Ok(UndoResult {
        info: info(doc, &g),
        offset,
    })
}

#[tauri::command]
pub fn redo(state: State<AppState>, doc: u32) -> R<UndoResult> {
    let d = get(&state, doc)?;
    let mut g = d.doc.write();
    let offset = g.table.redo();
    Ok(UndoResult {
        info: info(doc, &g),
        offset,
    })
}

// ---- clipboard ------------------------------------------------------------------

#[tauri::command]
pub fn copy_bytes(
    app: AppHandle,
    state: State<AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    format: CopyFormat,
    upper: bool,
) -> R<u64> {
    if len > util::MAX_COPY {
        return Err(format!(
            "selection is too large to copy ({} MiB max)",
            util::MAX_COPY >> 20
        ));
    }
    let d = get(&state, doc)?;
    let bytes = d.doc.read().table.read_vec(offset, len as usize);
    let text = util::format_bytes(&bytes, format, upper);
    app.clipboard().write_text(text).map_err(err)?;
    Ok(bytes.len() as u64)
}

#[tauri::command]
pub fn format_bytes(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    format: CopyFormat,
    upper: bool,
) -> R<String> {
    if len > util::MAX_COPY {
        return Err("selection is too large".into());
    }
    let d = get(&state, doc)?;
    let bytes = d.doc.read().table.read_vec(offset, len as usize);
    Ok(util::format_bytes(&bytes, format, upper))
}

#[derive(Serialize)]
pub struct PasteResult {
    info: DocInfo,
    len: u64,
}

/// Pastes text at `offset`, replacing `replace_len` bytes (the selection).
/// In overwrite mode with no selection, existing bytes are overwritten.
#[tauri::command]
pub fn paste(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    replace_len: u64,
    text: String,
    as_hex: bool,
    insert: bool,
) -> R<PasteResult> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let data = util::parse_paste(&text, as_hex).map_err(err)?;
    if data.is_empty() {
        return Err("clipboard is empty".into());
    }
    let mut g = d.doc.write();
    if replace_len > 0 {
        if insert {
            g.table.replace(offset, replace_len, &data);
        } else {
            // overwrite mode keeps the file size: write into the selection only
            let n = (data.len() as u64).min(replace_len) as usize;
            g.table.overwrite(offset, &data[..n]);
        }
    } else if insert {
        g.table.insert(offset, &data);
    } else {
        g.table.overwrite(offset, &data);
    }
    Ok(PasteResult {
        info: info(doc, &g),
        len: data.len() as u64,
    })
}

// ---- inspector ------------------------------------------------------------------

#[tauri::command]
pub fn inspect(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    little_endian: bool,
) -> R<Vec<Field>> {
    let d = get(&state, doc)?;
    let bytes = d.doc.read().table.read_vec(offset, 16);
    Ok(inspector::inspect(&bytes, little_endian))
}

#[tauri::command]
pub fn write_value(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    num_type: NumType,
    value: String,
    little_endian: bool,
) -> R<DocInfo> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let bytes = inspector::encode_value(num_type, &value, little_endian).map_err(err)?;
    let mut g = d.doc.write();
    if matches!(num_type, NumType::Uleb128 | NumType::Sleb128) {
        // variable length: replace the old encoding
        let old = g.table.read_vec(offset, 10);
        let old_len = match num_type {
            NumType::Uleb128 => inspector::read_uleb(&old).map(|x| x.1),
            _ => inspector::read_sleb(&old).map(|x| x.1),
        }
        .unwrap_or(0) as u64;
        g.table.replace(offset, old_len, &bytes);
    } else {
        g.table.overwrite(offset, &bytes);
    }
    Ok(info(doc, &g))
}

// ---- search -----------------------------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn find(
    app: AppHandle,
    state: State<'_, AppState>,
    doc: u32,
    query: Query,
    from: u64,
    backward: bool,
    wrap: bool,
    task_id: u32,
) -> R<Option<Match>> {
    let d = get(&state, doc)?;
    let m = Matcher::new(&query).map_err(err)?;
    run_task(app, &state, task_id, move |task| {
        let g = d.doc.read();
        if backward {
            m.find_prev(&g.table, from, wrap, task)
        } else {
            m.find_next(&g.table, from, wrap, task)
        }
    })
    .await
}

#[derive(Serialize)]
pub struct FindAll {
    matches: Vec<Match>,
    truncated: bool,
}

#[tauri::command]
pub async fn find_all(
    app: AppHandle,
    state: State<'_, AppState>,
    doc: u32,
    query: Query,
    limit: usize,
    task_id: u32,
) -> R<FindAll> {
    let d = get(&state, doc)?;
    let m = Matcher::new(&query).map_err(err)?;
    run_task(app, &state, task_id, move |task| {
        let (matches, truncated) = m.find_all(&d.doc.read().table, limit, task);
        FindAll { matches, truncated }
    })
    .await
}

#[tauri::command]
pub fn replace_one(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    query: Query,
    replacement: String,
) -> R<PasteResult> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let data = search::replacement_bytes(&query, &replacement).map_err(err)?;
    let mut g = d.doc.write();
    g.table.replace(offset, len, &data);
    Ok(PasteResult {
        info: info(doc, &g),
        len: data.len() as u64,
    })
}

#[derive(Serialize)]
pub struct ReplaceAll {
    info: DocInfo,
    count: usize,
}

#[tauri::command]
pub async fn replace_all(
    app: AppHandle,
    state: State<'_, AppState>,
    doc: u32,
    query: Query,
    replacement: String,
    task_id: u32,
) -> R<ReplaceAll> {
    let d = get(&state, doc)?;
    writable(&d)?;
    let m = Matcher::new(&query).map_err(err)?;
    let data = search::replacement_bytes(&query, &replacement).map_err(err)?;
    run_task(app, &state, task_id, move |task| {
        let mut g = d.doc.write();
        let (matches, _) = m.find_all(&g.table, 1_000_000, task);
        if task.cancelled() {
            return ReplaceAll {
                info: info(doc, &g),
                count: 0,
            };
        }
        search::replace_all(&mut g.table, &matches, &data);
        ReplaceAll {
            info: info(doc, &g),
            count: matches.len(),
        }
    })
    .await
}

// ---- analysis ---------------------------------------------------------------------

#[tauri::command]
pub async fn analyze_format(state: State<'_, AppState>, doc: u32) -> R<Arc<FormatInfo>> {
    let d = get(&state, doc)?;
    tauri::async_runtime::spawn_blocking(move || d.format())
        .await
        .map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisasmResult {
    arch: Option<Arch>,
    insns: Vec<Insn>,
}

#[tauri::command]
pub fn disassemble(
    state: State<AppState>,
    doc: u32,
    offset: u64,
    count: usize,
    arch: Option<Arch>,
) -> R<DisasmResult> {
    let d = get(&state, doc)?;
    let fmt = d.format_cached();
    let arch = arch.or(fmt.as_ref().and_then(|f| f.arch));
    let Some(arch) = arch else {
        return Ok(DisasmResult {
            arch: None,
            insns: vec![],
        });
    };
    let count = count.min(2000);
    let bytes = d.doc.read().table.read_vec(offset, count * arch.max_insn());
    let address = fmt
        .as_ref()
        .and_then(|f| f.offset_to_va(offset))
        .unwrap_or(offset);
    let insns = disasm::disassemble(arch, &bytes, offset, address, count).map_err(err)?;
    Ok(DisasmResult {
        arch: Some(arch),
        insns,
    })
}

#[tauri::command]
pub fn architectures() -> Vec<Arch> {
    Arch::ALL.to_vec()
}

#[derive(Serialize)]
pub struct StringsResult {
    strings: Vec<FoundString>,
    truncated: bool,
}

#[tauri::command]
pub async fn find_strings(
    app: AppHandle,
    state: State<'_, AppState>,
    doc: u32,
    options: StringsOptions,
    task_id: u32,
) -> R<StringsResult> {
    let d = get(&state, doc)?;
    run_task(app, &state, task_id, move |task| {
        let (strings, truncated) = analysis::strings(&d.doc.read().table, options, task);
        StringsResult { strings, truncated }
    })
    .await
}

#[tauri::command]
pub async fn entropy(
    app: AppHandle,
    state: State<'_, AppState>,
    doc: u32,
    blocks: usize,
    task_id: u32,
) -> R<EntropyMap> {
    let d = get(&state, doc)?;
    run_task(app, &state, task_id, move |task| {
        analysis::entropy_map(&d.doc.read().table, blocks, task)
    })
    .await
}

#[tauri::command]
pub async fn hashes(
    app: AppHandle,
    state: State<'_, AppState>,
    doc: u32,
    offset: u64,
    len: u64,
    task_id: u32,
) -> R<Vec<HashResult>> {
    let d = get(&state, doc)?;
    run_task(app, &state, task_id, move |task| {
        analysis::hashes(&d.doc.read().table, offset, len, task)
    })
    .await
}

#[tauri::command]
pub async fn next_difference(
    app: AppHandle,
    state: State<'_, AppState>,
    a: u32,
    b: u32,
    from: u64,
    backward: bool,
    task_id: u32,
) -> R<Option<u64>> {
    let (da, db) = (get(&state, a)?, get(&state, b)?);
    run_task(app, &state, task_id, move |task| {
        let (ga, gb) = (da.doc.read(), db.doc.read());
        if backward {
            analysis::prev_difference(&ga.table, &gb.table, from, task)
        } else {
            analysis::next_difference(&ga.table, &gb.table, from, task)
        }
    })
    .await
}

#[tauri::command]
pub async fn count_differences(
    app: AppHandle,
    state: State<'_, AppState>,
    a: u32,
    b: u32,
    task_id: u32,
) -> R<u64> {
    let (da, db) = (get(&state, a)?, get(&state, b)?);
    run_task(app, &state, task_id, move |task| {
        analysis::count_differences(&da.doc.read().table, &db.doc.read().table, task)
    })
    .await
}

// ---- addresses & expressions ------------------------------------------------------------

#[tauri::command]
pub fn va_to_offset(state: State<AppState>, doc: u32, va: u64) -> R<Option<u64>> {
    Ok(get(&state, doc)?
        .format_cached()
        .and_then(|f| f.va_to_offset(va)))
}

#[tauri::command]
pub fn offset_to_va(state: State<AppState>, doc: u32, offset: u64) -> R<Option<u64>> {
    Ok(get(&state, doc)?
        .format_cached()
        .and_then(|f| f.offset_to_va(offset)))
}

/// Evaluates a go-to expression. With `virtual_address`, the result is
/// interpreted as a VA and converted to a file offset.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn eval_offset(
    state: State<AppState>,
    doc: u32,
    expr: String,
    cursor: u64,
    sel_start: u64,
    sel_end: u64,
    hex_default: bool,
    virtual_address: bool,
) -> R<u64> {
    let d = get(&state, doc)?;
    let len = d.doc.read().len();
    let fmt = d.format_cached();
    let mut vars: HashMap<String, i128> = HashMap::new();
    for (k, v) in [
        ("end", len),
        ("size", len),
        ("eof", len),
        ("cur", cursor),
        ("here", cursor),
        ("pos", cursor),
        ("sel", sel_start),
        ("selstart", sel_start),
        ("selend", sel_end),
        ("sellen", sel_end.saturating_sub(sel_start)),
    ] {
        vars.insert(k.into(), v as i128);
    }
    if let Some(f) = &fmt {
        if let Some(e) = if virtual_address {
            f.entry
        } else {
            f.entry_offset
        } {
            vars.insert("entry".into(), e as i128);
        }
        for s in &f.symbols {
            let v = if virtual_address {
                Some(s.vaddr)
            } else {
                s.offset
            };
            if let Some(v) = v {
                vars.entry(format!("sym:{}", s.name)).or_insert(v as i128);
            }
        }
        for r in &f.regions {
            let v = if virtual_address {
                r.vaddr
            } else {
                Some(r.offset)
            };
            if let Some(v) = v {
                vars.entry(format!("sec:{}", r.name)).or_insert(v as i128);
            }
        }
    }
    let v = util::eval_expr(&expr, &vars, hex_default).map_err(err)?;
    if v < 0 {
        return Err("result is negative".into());
    }
    let v = u64::try_from(v).map_err(|_| "result is too large".to_string())?;
    if virtual_address {
        let f = fmt.ok_or("no executable format detected, virtual addresses are unavailable")?;
        return f
            .va_to_offset(v)
            .ok_or_else(|| format!("virtual address 0x{v:X} is not backed by file data"));
    }
    Ok(v)
}

// ---- annotations ----------------------------------------------------------------------

fn annotations_path(app: &AppHandle, doc: &Doc) -> R<PathBuf> {
    let g = doc.doc.read();
    let p = g.path().ok_or("save the file first to keep bookmarks")?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&p.to_string_lossy(), &mut h);
    let dir = app.path().app_data_dir().map_err(err)?.join("bookmarks");
    std::fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir.join(format!("{:016x}.json", std::hash::Hasher::finish(&h))))
}

#[tauri::command]
pub fn load_bookmarks(app: AppHandle, state: State<AppState>, doc: u32) -> R<serde_json::Value> {
    let d = get(&state, doc)?;
    let Ok(p) = annotations_path(&app, &d) else {
        return Ok(serde_json::Value::Array(vec![]));
    };
    match std::fs::read(&p) {
        Ok(b) => serde_json::from_slice::<serde_json::Value>(&b)
            .map(|v| {
                v.get("bookmarks")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![]))
            })
            .map_err(err),
        Err(_) => Ok(serde_json::Value::Array(vec![])),
    }
}

#[tauri::command]
pub fn save_bookmarks(
    app: AppHandle,
    state: State<AppState>,
    doc: u32,
    bookmarks: serde_json::Value,
) -> R<()> {
    let d = get(&state, doc)?;
    let p = annotations_path(&app, &d)?;
    let path = d
        .doc
        .read()
        .path()
        .map(|p| p.to_string_lossy().into_owned());
    let body = serde_json::json!({ "path": path, "bookmarks": bookmarks });
    std::fs::write(p, serde_json::to_vec_pretty(&body).map_err(err)?).map_err(err)
}
