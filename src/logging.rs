use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Rotate log files in `dir`: keep last 3 numbered backups.
/// debug.log.3 → deleted
/// debug.log.2 → debug.log.3
/// debug.log.1 → debug.log.2
/// debug.log   → debug.log.1
pub fn rotate_logs(dir: &Path) {
    const KEEP: u32 = 3;
    // Delete oldest
    let _ = std::fs::remove_file(dir.join(format!("debug.log.{}", KEEP)));
    // Shift numbered backups downward (highest first to avoid clobbering)
    for i in (1..KEEP).rev() {
        let from = dir.join(format!("debug.log.{}", i));
        let to = dir.join(format!("debug.log.{}", i + 1));
        let _ = std::fs::rename(from, to);
    }
    // Move current log to .1
    let _ = std::fs::rename(dir.join("debug.log"), dir.join("debug.log.1"));
}

/// Generate a 4-hex-char random instance ID for log file naming.
/// Uses std::hash::RandomState which is randomly seeded per process.
fn generate_instance_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_usize(std::process::id() as usize);
    format!("{:04x}", hasher.finish() as u16)
}

/// Rotate diagnostic log files: keep the 6 most recent by mtime.
/// Different from `rotate_logs` which uses numbered backups for a single file.
pub fn rotate_diagnostic_logs(dir: &Path) {
    const KEEP: usize = 6;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("diagnostic-") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), mtime))
        })
        .collect();

    if entries.len() <= KEEP {
        return;
    }

    // Sort newest first
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    // Remove everything beyond the 6th
    for (path, _) in &entries[KEEP..] {
        let _ = std::fs::remove_file(path);
    }
}

/// Logging init result — holds worker guards and optional instance ID.
pub struct LoggingGuards {
    /// MUST be held for the lifetime of main. Dropping flushes and closes writers.
    pub guards: Vec<WorkerGuard>,
    /// 4-hex-char instance ID when diagnostic mode is active, for span injection.
    pub instance_id: Option<String>,
}

/// Initialise tracing.
///
/// - `debug`: enables both DEBUG-level file logging to `.codescout/debug.log`
///   and INFO-level diagnostic logging to `.codescout/diagnostic-<hash>.log`.
///
/// Returns guards that MUST be held for the lifetime of `main`, plus the
/// diagnostic instance ID (if active) for root span injection.
pub fn init(debug: bool) -> LoggingGuards {
    let mut guards = Vec::new();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));

    let log_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".codescout");

    if debug {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("codescout: could not create log directory: {e}");
        }
    }

    // --- Debug file layer (DEBUG level) ---
    let debug_layer = if debug {
        rotate_logs(&log_dir);
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_dir.join("debug.log"))
        {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_filter(EnvFilter::new("debug")),
                )
            }
            Err(e) => {
                eprintln!("codescout: could not open debug log: {e}");
                None
            }
        }
    } else {
        None
    };

    // --- Diagnostic file layer (INFO level) ---
    let mut instance_id = None;
    let diagnostic_layer = if debug {
        rotate_diagnostic_logs(&log_dir);
        let id = generate_instance_id();
        let filename = format!("diagnostic-{id}.log");
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_dir.join(&filename))
        {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);
                instance_id = Some(id);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_filter(EnvFilter::new("info")),
                )
            }
            Err(e) => {
                eprintln!("codescout: could not open diagnostic log {filename}: {e}");
                None
            }
        }
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(debug_layer)
        .with(diagnostic_layer)
        .try_init()
        .ok();

    LoggingGuards {
        guards,
        instance_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_diagnostic_keeps_last_6() {
        let dir = tempfile::tempdir().unwrap();
        // Create 8 diagnostic files with staggered mtimes
        for i in 0..8 {
            let path = dir.path().join(format!("diagnostic-{:04x}.log", i));
            std::fs::write(&path, format!("log {i}")).unwrap();
            let mtime = filetime::FileTime::from_unix_time(1000 + i as i64, 0);
            filetime::set_file_mtime(&path, mtime).unwrap();
        }

        super::rotate_diagnostic_logs(dir.path());

        let mut remaining: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        remaining.sort();
        assert_eq!(
            remaining.len(),
            6,
            "should keep exactly 6 files: {remaining:?}"
        );
        // The two oldest (0000 and 0001) should be deleted
        assert!(!remaining.contains(&"diagnostic-0000.log".to_string()));
        assert!(!remaining.contains(&"diagnostic-0001.log".to_string()));
    }

    #[test]
    fn rotate_diagnostic_ignores_non_diagnostic_files() {
        let dir = tempfile::tempdir().unwrap();
        // Create 8 diagnostic files + 3 non-diagnostic files
        for i in 0..8 {
            let path = dir.path().join(format!("diagnostic-{:04x}.log", i));
            std::fs::write(&path, format!("log {i}")).unwrap();
            let mtime = filetime::FileTime::from_unix_time(1000 + i as i64, 0);
            filetime::set_file_mtime(&path, mtime).unwrap();
        }
        std::fs::write(dir.path().join("debug.log"), "debug").unwrap();
        std::fs::write(dir.path().join("debug.log.1"), "debug old").unwrap();
        std::fs::write(dir.path().join("random.txt"), "other").unwrap();

        super::rotate_diagnostic_logs(dir.path());

        let all: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        // 6 diagnostic + 3 non-diagnostic = 9
        assert_eq!(
            all.len(),
            9,
            "non-diagnostic files must be untouched: {all:?}"
        );
    }

    #[test]
    fn generate_instance_id_is_4_hex_chars() {
        let id = super::generate_instance_id();
        assert_eq!(id.len(), 4, "instance ID must be 4 chars: got '{id}'");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "instance ID must be hex: got '{id}'"
        );
    }

    #[test]
    fn generate_instance_id_varies_across_calls() {
        // RandomState is randomly seeded, so two calls should differ.
        // There's a 1/65536 chance of collision — acceptable for a test.
        let a = super::generate_instance_id();
        let b = super::generate_instance_id();
        assert_ne!(a, b, "instance IDs should vary across calls");
    }

    #[test]
    fn rotate_keeps_last_3() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();

        // Populate 4 log files with their own name as content (for verification)
        for name in &["debug.log", "debug.log.1", "debug.log.2", "debug.log.3"] {
            std::fs::write(p.join(name), name.as_bytes()).unwrap();
        }

        rotate_logs(p);

        // Original debug.log.3 is deleted — no debug.log.4 should exist
        assert!(!p.join("debug.log.4").exists());
        // debug.log.3 now contains original debug.log.2 content
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.3")).unwrap(),
            "debug.log.2"
        );
        // debug.log.2 now contains original debug.log.1 content
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.2")).unwrap(),
            "debug.log.1"
        );
        // debug.log.1 now contains original debug.log content
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.1")).unwrap(),
            "debug.log"
        );
        // debug.log itself is gone (renamed to .1)
        assert!(!p.join("debug.log").exists());
    }

    #[test]
    fn rotate_works_when_no_files_exist() {
        let dir = tempfile::tempdir().unwrap();
        rotate_logs(dir.path()); // Must not panic
    }

    #[test]
    fn rotate_works_with_only_current_log() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::write(p.join("debug.log"), b"hello").unwrap();
        rotate_logs(p);
        assert!(!p.join("debug.log").exists());
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.1")).unwrap(),
            "hello"
        );
    }
}
