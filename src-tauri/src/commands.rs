//! Tauri command layer: thin wrappers around `pipeline`, plus batch orchestration.

use crate::pipeline::{
    self, ProcessOptions, ProcessOutcome, SUPPORTED_INPUT_EXTENSIONS,
};
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// Set while a batch runs so the UI can request a stop.
#[derive(Default)]
pub struct CancelFlag(pub AtomicBool);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageEntry {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileProgress {
    pub path: String,
    pub file_name: String,
    pub completed: usize,
    pub total: usize,
    pub status: String,
    pub error: Option<String>,
    pub original_bytes: u64,
    pub output_bytes: u64,
    pub output_width: u32,
    pub output_height: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub processed: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub total_original_bytes: u64,
    pub total_output_bytes: u64,
}

/// Lets the settings panel offer meaningful performance choices instead of raw thread counts.
#[tauri::command]
pub fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[tauri::command]
pub fn list_images_in_folder(
    folder: String,
    recursive: bool,
    exclude_dir: Option<String>,
) -> Result<Vec<ImageEntry>, String> {
    let excluded = normalized_exclusion(exclude_dir);
    let mut entries = Vec::new();
    collect_images(Path::new(&folder), recursive, excluded.as_deref(), &mut entries)?;
    entries.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    Ok(entries)
}

/// Accepts whatever was dropped on the window: individual images, folders, or a mix.
#[tauri::command]
pub fn collect_dropped_paths(
    paths: Vec<String>,
    recursive: bool,
    exclude_dir: Option<String>,
) -> Result<Vec<ImageEntry>, String> {
    let excluded = normalized_exclusion(exclude_dir);
    let mut entries = Vec::new();

    for raw in &paths {
        let path = Path::new(raw);
        if path.is_dir() {
            collect_images(path, recursive, excluded.as_deref(), &mut entries)?;
        } else if is_supported_image(path) {
            if let Some(entry) = image_entry(path) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    entries.dedup_by(|a, b| a.path == b.path);
    Ok(entries)
}

/// Canonicalized so an output folder nested in the input folder is recognised whatever
/// separators or casing the UI passed down.
fn normalized_exclusion(exclude_dir: Option<String>) -> Option<PathBuf> {
    let dir = exclude_dir?;
    if dir.is_empty() {
        return None;
    }
    Some(std::fs::canonicalize(&dir).unwrap_or_else(|_| PathBuf::from(dir)))
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_INPUT_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn image_entry(path: &Path) -> Option<ImageEntry> {
    Some(ImageEntry {
        file_name: path.file_name().and_then(|n| n.to_str())?.to_string(),
        size_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        path: path.to_string_lossy().to_string(),
    })
}

fn collect_images(
    dir: &Path,
    recursive: bool,
    exclude_dir: Option<&Path>,
    out: &mut Vec<ImageEntry>,
) -> Result<(), String> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_dir() {
            // Writing results into a subfolder of the input is normal; re-reading them on
            // the next scan and processing our own output is not.
            let is_excluded = exclude_dir
                .map(|excluded| {
                    std::fs::canonicalize(&path)
                        .map(|resolved| resolved == excluded)
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            if recursive && !is_excluded {
                collect_images(&path, recursive, exclude_dir, out)?;
            }
            continue;
        }

        if is_supported_image(&path) {
            if let Some(found) = image_entry(&path) {
                out.push(found);
            }
        }
    }

    Ok(())
}

/// Holds the last previewed source, decoded, together with its rendered "before" image.
/// Only one entry: the user looks at one photo at a time, and a decoded 12MP frame is
/// ~34MB, which is not something to keep several of.
pub struct CachedSource {
    path: PathBuf,
    display_size: u32,
    image: image::DynamicImage,
    before_uri: String,
}

#[derive(Default)]
pub struct PreviewCache(pub Mutex<Option<CachedSource>>);

/// Runs on a blocking thread: a 12MP decode plus a real encode is far too slow for the
/// UI thread, and the user triggers this on every settings change.
#[tauri::command]
pub async fn preview_file(
    cache: State<'_, PreviewCache>,
    path: String,
    options: ProcessOptions,
    display_size: u32,
) -> Result<pipeline::PreviewResult, String> {
    let source = PathBuf::from(&path);

    let cached = {
        let guard = cache.0.lock().map_err(|_| "preview cache poisoned")?;
        match guard.as_ref() {
            Some(entry) if entry.path == source && entry.display_size == display_size => {
                Some((entry.image.clone(), entry.before_uri.clone()))
            }
            _ => None,
        }
    };

    let (image, before_uri) = match cached {
        Some(entry) => entry,
        None => {
            let for_decode = source.clone();
            let (decoded, before_uri) = tauri::async_runtime::spawn_blocking(move || {
                let decoded = pipeline::load_oriented(&for_decode)?;
                let before_uri = pipeline::source_preview_uri(&decoded, display_size)?;
                Ok::<_, String>((decoded, before_uri))
            })
            .await
            .map_err(|e| format!("decode task failed: {e}"))??;

            let mut guard = cache.0.lock().map_err(|_| "preview cache poisoned")?;
            *guard = Some(CachedSource {
                path: source.clone(),
                display_size,
                image: decoded.clone(),
                before_uri: before_uri.clone(),
            });
            (decoded, before_uri)
        }
    };

    tauri::async_runtime::spawn_blocking(move || {
        let watermark = pipeline::load_watermark(&options)?;
        pipeline::preview_from_image(
            &image,
            &source,
            &options,
            display_size,
            Some(before_uri),
            watermark.as_ref(),
        )
    })
    .await
    .map_err(|e| format!("preview task failed: {e}"))?
}

#[tauri::command]
pub fn cancel_batch(cancel: State<'_, CancelFlag>) {
    cancel.0.store(true, Ordering::Relaxed);
}

/// Every worker holds one fully decoded image, so parallelism is also the memory dial:
/// roughly `threads x width x height x 3` bytes at peak.
fn resolve_thread_count(requested: Option<usize>) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    requested.unwrap_or(cores).clamp(1, cores)
}

#[tauri::command]
pub async fn process_batch(
    app: AppHandle,
    cancel: State<'_, CancelFlag>,
    files: Vec<String>,
    output_dir: String,
    options: ProcessOptions,
    max_parallel: Option<usize>,
) -> Result<BatchSummary, String> {
    let output_dir = PathBuf::from(&output_dir);
    std::fs::create_dir_all(&output_dir).map_err(|e| format!("cannot create output dir: {e}"))?;

    cancel.0.store(false, Ordering::Relaxed);

    let total = files.len();
    let completed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let original_bytes = AtomicUsize::new(0);
    let output_bytes = AtomicUsize::new(0);

    // Decoded once for the whole run rather than once per image.
    let watermark = pipeline::load_watermark(&options)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(resolve_thread_count(max_parallel))
        .build()
        .map_err(|e| format!("cannot start worker pool: {e}"))?;

    pool.install(|| {
        files.par_iter().for_each(|path| {
            if cancel.0.load(Ordering::Relaxed) {
                return;
            }

            let source = Path::new(path);
            let file_name = source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string();

            let result =
                pipeline::process_file(source, &output_dir, &options, watermark.as_ref());
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;

            let progress = match &result {
                Ok(outcome) => {
                    original_bytes.fetch_add(outcome.original_bytes as usize, Ordering::Relaxed);
                    output_bytes.fetch_add(outcome.output_bytes as usize, Ordering::Relaxed);
                    success_progress(path, &file_name, done, total, outcome)
                }
                Err(error) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    failure_progress(path, &file_name, done, total, error)
                }
            };

            let _ = app.emit("batch-progress", progress);
        })
    });

    let cancelled = cancel.0.load(Ordering::Relaxed);
    let failed = failed.load(Ordering::Relaxed);

    Ok(BatchSummary {
        processed: completed.load(Ordering::Relaxed) - failed,
        failed,
        cancelled,
        total_original_bytes: original_bytes.load(Ordering::Relaxed) as u64,
        total_output_bytes: output_bytes.load(Ordering::Relaxed) as u64,
    })
}

fn success_progress(
    path: &str,
    file_name: &str,
    completed: usize,
    total: usize,
    outcome: &ProcessOutcome,
) -> FileProgress {
    FileProgress {
        path: path.to_string(),
        file_name: file_name.to_string(),
        completed,
        total,
        status: "done".into(),
        error: None,
        original_bytes: outcome.original_bytes,
        output_bytes: outcome.output_bytes,
        output_width: outcome.output_width,
        output_height: outcome.output_height,
    }
}

fn failure_progress(
    path: &str,
    file_name: &str,
    completed: usize,
    total: usize,
    error: &str,
) -> FileProgress {
    FileProgress {
        path: path.to_string(),
        file_name: file_name.to_string(),
        completed,
        total,
        status: "error".into(),
        error: Some(error.to_string()),
        original_bytes: 0,
        output_bytes: 0,
        output_width: 0,
        output_height: 0,
    }
}
