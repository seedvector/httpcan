//! Non-blocking stderr logging.
//!
//! `env_logger` writes every record synchronously: a global lock plus a
//! `write(2)` per line, executed on the request's worker thread. Under load
//! that serializes workers on the logger and costs ~4x throughput.
//! This module keeps the `RUST_LOG`
//! semantics (via `env_filter`, the same directive parser env_logger uses)
//! and the familiar `[<ts> LEVEL  target] message` line format, but hands
//! formatted lines to a bounded queue drained by a dedicated writer thread:
//!
//! - the request path never blocks on stderr: when the queue is full the
//!   line is dropped and counted (the writer thread reports the loss);
//! - lines are batched and flushed by the writer thread.
//!
//! Trade-off: log lines surface within ~[`POLL_INTERVAL`] (1 ms) and on
//! process exit at most one writer-buffer's worth of trailing lines may
//! be lost. Acceptable for access logs.

use std::fmt::Write as _;
use std::io::{BufWriter, IsTerminal, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use env_filter::Filter;
use log::{Level, LevelFilter, Log, Metadata, Record};

/// How often the writer thread wakes to drain the queue. Also the upper
/// bound on log-line latency.
const POLL_INTERVAL: Duration = Duration::from_millis(1);
/// Queue depth: burst absorption before lines start dropping.
const QUEUE_CAPACITY: usize = 16 * 1024;

/// `log` facade backend: filter plus bounded hand-off to the writer thread.
pub struct AsyncStderrLogger {
    filter: Filter,
    tx: Mutex<SyncSender<String>>,
    dropped: Arc<AtomicU64>,
    color: bool,
}

impl AsyncStderrLogger {
    /// Build a logger from a filter, returning the queue to drain and the
    /// shared drop counter (kept separate for testability; [`init`] wires
    /// both to the writer thread).
    fn with_filter(filter: Filter) -> (Self, Receiver<String>, Arc<AtomicU64>) {
        let (tx, rx) = sync_channel(QUEUE_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        (
            Self {
                filter,
                tx: Mutex::new(tx),
                dropped: Arc::clone(&dropped),
                color: std::io::stderr().is_terminal(),
            },
            rx,
            dropped,
        )
    }
}

impl Log for AsyncStderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if !self.filter.matches(record) {
            return;
        }
        let mut line = String::with_capacity(160);
        format_line(&mut line, record, self.color);
        let tx = self
            .tx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Err(TrySendError::Full(_)) = tx.try_send(line) {
            // Writer thread is behind: drop instead of blocking the worker.
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {
        // Flushing is the writer thread's job (at least every
        // FLUSH_INTERVAL); nothing to do on the producer side.
    }
}

/// Append one line in env_logger's default format:
/// `[2026-08-20T00:44:47Z INFO  httpcan::middleware] message`.
/// The level is colorized when stderr is a terminal.
fn format_line(out: &mut String, record: &Record, color: bool) {
    out.push('[');
    let _ = write!(out, "{}", Utc::now().format("%Y-%m-%dT%H:%M:%SZ"));
    out.push(' ');
    let level = record.level().as_str();
    if color {
        out.push_str(level_ansi(record.level()));
    }
    out.push_str(level);
    for _ in level.len()..5 {
        out.push(' ');
    }
    if color {
        out.push_str("\x1b[0m");
    }
    out.push(' ');
    out.push_str(record.target());
    out.push_str("] ");
    let _ = write!(out, "{}", record.args());
}

/// ANSI color for a level, matching env_logger's palette.
fn level_ansi(level: Level) -> &'static str {
    match level {
        Level::Error => "\x1b[31m",
        Level::Warn => "\x1b[33m",
        Level::Info => "\x1b[32m",
        Level::Debug => "\x1b[34m",
        Level::Trace => "\x1b[36m",
    }
}

/// Drain the queue and write to stderr in batches, reporting dropped lines.
///
/// The writer polls instead of parking on the channel: a parked std
/// channel receiver gets futex-woken on *every* send, which turns each
/// producer `try_send` into a wakeup ping-pong (~tens of microseconds per
/// line - dominant when one worker logs at full rate, e.g. a single
/// keep-alive connection dropped t1c1 from 85k to 14k RPS before this
/// change). Polling keeps the producer path pure userspace; lines surface
/// within [`POLL_INTERVAL`] and flush with the next batch.
fn run_writer(rx: Receiver<String>, dropped: Arc<AtomicU64>) {
    let stderr = std::io::stderr();
    let mut out = BufWriter::with_capacity(128 * 1024, stderr.lock());
    let mut reported: u64 = 0;
    loop {
        thread::sleep(POLL_INTERVAL);
        let mut wrote = false;
        while let Ok(line) = rx.try_recv() {
            let _ = out.write_all(line.as_bytes());
            let _ = out.write_all(b"\n");
            wrote = true;
        }
        let current = dropped.load(Ordering::Relaxed);
        if current > reported {
            let _ = writeln!(
                out,
                "[{} WARN  httpcan::logging] {} log lines dropped (stderr writer behind)",
                Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                current - reported
            );
            wrote = true;
            reported = current;
        }
        if wrote {
            let _ = out.flush();
        }
    }
}
/// Install the global logger. `RUST_LOG` keeps env_logger's directive
/// syntax (e.g. `httpcan=debug,actix_server=off`); the default level is
/// `info`, so per-request access logs stay visible out of the box.
pub fn init() {
    let mut builder = env_filter::Builder::new();
    match std::env::var("RUST_LOG") {
        Ok(spec) if !spec.is_empty() => {
            builder.parse(&spec);
        }
        _ => {
            builder.filter_level(LevelFilter::Info);
        }
    }
    let filter = builder.build();
    let max_level = filter.filter();
    let (logger, rx, dropped) = AsyncStderrLogger::with_filter(filter);
    thread::Builder::new()
        .name("httpcan-log-writer".to_owned())
        .spawn(move || run_writer(rx, dropped))
        .expect("spawn log writer thread");
    log::set_boxed_logger(Box::new(logger)).expect("install httpcan logger once");
    log::set_max_level(max_level);
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level;

    #[test]
    fn format_line_matches_env_logger_shape() {
        let record = Record::builder()
            .args(format_args!("time=x level=info"))
            .level(Level::Info)
            .target("httpcan::middleware")
            .build();
        let mut line = String::new();
        format_line(&mut line, &record, false);
        // [2026-08-20T00:44:47Z INFO  httpcan::middleware] time=x level=info
        assert!(line.starts_with('['), "{line}");
        let (ts, rest) = line[1..].split_once(' ').expect("space after timestamp");
        assert_eq!(ts.len(), 20, "RFC3339 seconds timestamp, got {ts}");
        assert_eq!(
            rest, "INFO  httpcan::middleware] time=x level=info",
            "level padded to 5, single space before target"
        );
    }

    #[test]
    fn format_line_pads_short_levels() {
        let record = Record::builder()
            .args(format_args!("m"))
            .level(Level::Warn)
            .target("t")
            .build();
        let mut line = String::new();
        format_line(&mut line, &record, false);
        assert!(line.contains(" WARN  t] m"), "WARN padded to 5: {line}");
    }

    #[test]
    fn filter_honors_rust_log_directives() {
        let error_only = env_filter::Builder::new().parse("error").build();
        let info = Record::builder()
            .args(format_args!("x"))
            .level(Level::Info)
            .target("httpcan")
            .build();
        let error = Record::builder()
            .args(format_args!("x"))
            .level(Level::Error)
            .target("httpcan")
            .build();
        assert!(!error_only.matches(&info));
        assert!(error_only.matches(&error));

        let module = env_filter::Builder::new()
            .parse("info,actix_server=off")
            .build();
        let actix = Record::builder()
            .args(format_args!("x"))
            .level(Level::Error)
            .target("actix_server::server")
            .build();
        assert!(module.matches(&info));
        assert!(!module.matches(&actix), "module directives must apply");
    }

    #[test]
    fn queue_overflow_drops_instead_of_blocking() {
        // Regression: the sync logger
        // blocked workers on stderr; the async one must drop and count
        // when the writer thread cannot keep up.
        let filter = env_filter::Builder::new().parse("trace").build();
        let (logger, _rx, dropped) = AsyncStderrLogger::with_filter(filter);
        // No writer draining: fill the queue, then overflow by two.
        for _ in 0..QUEUE_CAPACITY {
            let record = Record::builder()
                .args(format_args!("filler"))
                .level(Level::Info)
                .target("t")
                .build();
            logger.log(&record);
        }
        assert_eq!(dropped.load(Ordering::Relaxed), 0, "queue absorbs a full batch");
        for _ in 0..2 {
            let record = Record::builder()
                .args(format_args!("filler"))
                .level(Level::Info)
                .target("t")
                .build();
            logger.log(&record);
        }
        assert_eq!(dropped.load(Ordering::Relaxed), 2, "overflow is dropped, not blocked");
    }
}
