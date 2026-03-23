use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

/// Session-scoped tracking of which markdown sections have been read.
#[derive(Debug, Default)]
pub struct SectionCoverage {
    seen: HashMap<PathBuf, HashSet<String>>,
    mtimes: HashMap<PathBuf, SystemTime>,
}

pub struct CoverageStatus {
    pub read_count: usize,
    pub total_count: usize,
    pub unread: Vec<String>,
}

impl SectionCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_seen(&mut self, path: &PathBuf, headings: &[String]) {
        let entry = self.seen.entry(path.clone()).or_default();
        for h in headings {
            entry.insert(h.clone());
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                self.mtimes.insert(path.clone(), mtime);
            }
        }
    }

    pub fn update_mtime(&mut self, path: &PathBuf) {
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                self.mtimes.insert(path.clone(), mtime);
            }
        }
    }

    fn validate(&mut self, path: &PathBuf) -> bool {
        if !self.seen.contains_key(path) {
            return false;
        }
        if let Some(stored_mtime) = self.mtimes.get(path) {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(current_mtime) = meta.modified() {
                    if current_mtime != *stored_mtime {
                        self.seen.remove(path);
                        self.mtimes.remove(path);
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn status(&mut self, path: &PathBuf, all_headings: &[String]) -> Option<CoverageStatus> {
        if !self.validate(path) {
            return None;
        }
        let seen = self.seen.get(path)?;
        let unread: Vec<String> = all_headings
            .iter()
            .filter(|h| !seen.contains(*h))
            .cloned()
            .collect();
        Some(CoverageStatus {
            read_count: all_headings.len() - unread.len(),
            total_count: all_headings.len(),
            unread,
        })
    }

    pub fn unread_hint(&mut self, path: &PathBuf, all_headings: &[String]) -> Option<String> {
        let status = self.status(path, all_headings)?;
        if status.unread.is_empty() {
            return None;
        }
        let preview: Vec<&str> = status.unread.iter().map(|s| s.as_str()).take(5).collect();
        let suffix = if status.unread.len() > 5 {
            format!(", ... ({} more)", status.unread.len() - 5)
        } else {
            String::new()
        };
        Some(format!(
            "{} unread sections in this file: {}{}",
            status.unread.len(),
            preview.join(", "),
            suffix
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_temp_file(content: &str) -> (NamedTempFile, PathBuf) {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn mark_seen_and_status() {
        let (_f, path) = make_temp_file("# Title\n## A\n## B\n## C\n");
        let mut cov = SectionCoverage::new();
        let all = vec![
            "# Title".into(),
            "## A".into(),
            "## B".into(),
            "## C".into(),
        ];
        assert!(cov.status(&path, &all).is_none());
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);
        let s = cov.status(&path, &all).unwrap();
        assert_eq!(s.read_count, 2);
        assert_eq!(s.total_count, 4);
        assert_eq!(s.unread, vec!["## B", "## C"]);
    }

    #[test]
    fn mark_all_seen_no_unread() {
        let (_f, path) = make_temp_file("# Title\n## A\n");
        let mut cov = SectionCoverage::new();
        let all = vec!["# Title".into(), "## A".into()];
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);
        let s = cov.status(&path, &all).unwrap();
        assert!(s.unread.is_empty());
    }

    #[test]
    fn mtime_invalidation() {
        let (f, path) = make_temp_file("# Title\n## A\n");
        let mut cov = SectionCoverage::new();
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "# Title\n## A\n## B\n").unwrap();
        let all = vec!["# Title".into(), "## A".into(), "## B".into()];
        assert!(cov.status(&path, &all).is_none());
        drop(f);
    }

    #[test]
    fn update_mtime_prevents_invalidation() {
        let (_f, path) = make_temp_file("# Title\n## A\n");
        let mut cov = SectionCoverage::new();
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "# Title\n## A modified\n").unwrap();
        cov.update_mtime(&path);
        let all = vec!["# Title".into(), "## A".into()];
        assert!(cov.status(&path, &all).is_some());
    }

    #[test]
    fn unread_hint_format() {
        let (_f, path) = make_temp_file("x");
        let mut cov = SectionCoverage::new();
        cov.mark_seen(&path, &["# Title".into()]);
        let all: Vec<String> = std::iter::once("# Title".to_string())
            .chain((0..7).map(|i| format!("## Section {i}")))
            .collect();
        let hint = cov.unread_hint(&path, &all).unwrap();
        assert!(hint.contains("7 unread"));
        assert!(hint.contains("2 more"));
    }

    #[test]
    fn no_hint_when_no_coverage() {
        let (_f, path) = make_temp_file("x");
        let mut cov = SectionCoverage::new();
        let all = vec!["## A".into()];
        assert!(cov.unread_hint(&path, &all).is_none());
    }
}
