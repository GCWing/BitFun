use std::fs;
use std::path::PathBuf;

use crate::error::Result;

/// 状态快照管理器
pub struct StateManager {
    snapshot_dir: PathBuf,
    max_keep: usize,
}

impl StateManager {
    pub fn new(snapshot_dir: PathBuf) -> Self {
        fs::create_dir_all(&snapshot_dir).ok();
        Self {
            snapshot_dir,
            max_keep: 10,
        }
    }

    /// 保存快照（序列化 StateStore 到 JSON 文件）
    pub fn save(&self, state_json: &str, version: &str) -> Result<()> {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("snapshot_{}_v{}.json", ts, version);
        let path = self.snapshot_dir.join(&filename);
        fs::write(&path, state_json)?;
        self.cleanup()?;
        Ok(())
    }

    /// 列出所有快照文件（按时间倒序）
    pub fn list_snapshots(&self) -> Result<Vec<PathBuf>> {
        let mut entries: Vec<PathBuf> = Vec::new();
        if let Ok(dir) = fs::read_dir(&self.snapshot_dir) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    entries.push(path);
                }
            }
        }
        entries.sort_by(|a, b| b.cmp(a)); // newest first
        Ok(entries)
    }

    /// 加载最近的快照
    pub fn load(&self) -> Result<Option<String>> {
        let snapshots = self.list_snapshots()?;
        if let Some(latest) = snapshots.first() {
            let content = fs::read_to_string(latest)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// 清理旧快照（保留最近 N 个）
    fn cleanup(&self) -> Result<()> {
        let snapshots = self.list_snapshots()?;
        for old in snapshots.iter().skip(self.max_keep) {
            fs::remove_file(old)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_save_and_list() {
        let dir = env::temp_dir().join("taiji_test_snapshots");
        let mgr = StateManager::new(dir.clone());
        mgr.save(r#"{"test": true}"#, "1.0").unwrap();
        let list = mgr.list_snapshots().unwrap();
        assert!(!list.is_empty());

        // Cleanup
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_empty() {
        let dir = env::temp_dir().join("taiji_test_empty");
        let mgr = StateManager::new(dir.clone());
        let result = mgr.load().unwrap();
        assert!(result.is_none());
        fs::remove_dir_all(&dir).ok();
    }
}
