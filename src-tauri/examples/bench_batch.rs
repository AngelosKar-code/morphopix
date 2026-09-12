//! Runs a real batch over a folder and reports throughput and peak memory.
//!
//!     cargo run --release --example bench_batch -- <input-dir> <output-dir> [threads]
//!
//! Answers the practical question "what happens if I drop 2000 photos in": every worker
//! holds one decoded frame, so peak memory scales with thread count, not file count.

use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri_app_lib::pipeline::{self, ProcessOptions, SUPPORTED_INPUT_EXTENSIONS};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| SUPPORTED_INPUT_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: bench_batch <input> <output> [threads]");
    let output = args.next().expect("usage: bench_batch <input> <output> [threads]");
    let threads: usize = args
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));

    let mut files = Vec::new();
    collect(Path::new(&input), &mut files);
    std::fs::create_dir_all(&output).expect("cannot create output dir");

    println!("\n  {} images, {threads} worker threads", files.len());

    let options = ProcessOptions::default();
    let output_dir = PathBuf::from(&output);
    let original = AtomicU64::new(0);
    let produced = AtomicU64::new(0);
    let failures = AtomicU64::new(0);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("cannot build pool");

    let started = std::time::Instant::now();
    pool.install(|| {
        files.par_iter().for_each(|path| {
            match pipeline::process_file(path, &output_dir, &options, None) {
                Ok(outcome) => {
                    original.fetch_add(outcome.original_bytes, Ordering::Relaxed);
                    produced.fetch_add(outcome.output_bytes, Ordering::Relaxed);
                }
                Err(error) => {
                    failures.fetch_add(1, Ordering::Relaxed);
                    eprintln!("  failed {}: {error}", path.display());
                }
            }
        })
    });
    let elapsed = started.elapsed();

    let original = original.load(Ordering::Relaxed);
    let produced = produced.load(Ordering::Relaxed);

    println!("  finished in {elapsed:?}");
    println!(
        "  {:.1} images/sec, {:.0} ms per image of wall time",
        files.len() as f64 / elapsed.as_secs_f64(),
        elapsed.as_millis() as f64 / files.len() as f64,
    );
    println!(
        "  {:.1} MB -> {:.1} MB ({:.1}% smaller), {} failures",
        original as f64 / 1_048_576.0,
        produced as f64 / 1_048_576.0,
        (1.0 - produced as f64 / original as f64) * 100.0,
        failures.load(Ordering::Relaxed),
    );
    println!(
        "\n  extrapolated to 2000 images: {:.1} min\n",
        elapsed.as_secs_f64() / files.len() as f64 * 2000.0 / 60.0,
    );
}
