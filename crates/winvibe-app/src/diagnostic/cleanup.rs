use std::path::Path;
use std::time::{Duration, SystemTime};

pub fn cleanup_old_diagnostic_files(dir: &Path, max_age: Duration) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "jsonl") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = match metadata.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if SystemTime::now().duration_since(modified).unwrap_or_default() > max_age {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!(?path, error = %e, "清理过期 diagnostic 文件失败");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_removes_old_diagnostic_files() {
        let dir = tempfile::tempdir().unwrap();
        let old_file = dir.path().join("old-approval.jsonl");
        std::fs::write(&old_file, "{}").unwrap();
        let old_time = SystemTime::now() - Duration::from_secs(8 * 24 * 3600);
        filetime::set_file_mtime(
            &old_file,
            filetime::FileTime::from_system_time(old_time),
        ).unwrap();
        cleanup_old_diagnostic_files(dir.path(), Duration::from_secs(7 * 24 * 3600));
        assert!(!old_file.exists());
    }

    #[test]
    fn cleanup_keeps_recent_diagnostic_files() {
        let dir = tempfile::tempdir().unwrap();
        let recent = dir.path().join("recent-approval.jsonl");
        std::fs::write(&recent, "{}").unwrap();
        cleanup_old_diagnostic_files(dir.path(), Duration::from_secs(7 * 24 * 3600));
        assert!(recent.exists());
    }
}
