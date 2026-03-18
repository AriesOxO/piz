use anyhow::{Context, Result};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::config;

/// A history entry: (query, command, exit_code, danger, created_at)
pub type HistoryEntry = (String, String, i32, String, u64);

pub struct Cache {
    conn: Connection,
    ttl_hours: u64,
    max_entries: usize,
}

impl Cache {
    pub fn open(ttl_hours: u64) -> Result<Self> {
        Self::open_with_max(ttl_hours, 1000)
    }

    pub fn open_with_max(ttl_hours: u64, max_entries: usize) -> Result<Self> {
        let dir = config::piz_dir()?;
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("cache.db");
        Self::open_at(&db_path, ttl_hours, max_entries)
    }

    pub fn open_at(db_path: &std::path::Path, ttl_hours: u64, max_entries: usize) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open cache db: {}", db_path.display()))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache (
                key TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                danger TEXT NOT NULL,
                explanation TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                command TEXT NOT NULL,
                exit_code INTEGER NOT NULL,
                danger TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
        )?;

        // Migration: add explanation column to existing databases
        let has_explanation = conn
            .prepare("SELECT explanation FROM cache LIMIT 0")
            .is_ok();
        if !has_explanation {
            conn.execute_batch(
                "ALTER TABLE cache ADD COLUMN explanation TEXT NOT NULL DEFAULT '';",
            )?;
        }

        let cache = Self {
            conn,
            ttl_hours,
            max_entries,
        };
        cache.evict_expired()?;
        Ok(cache)
    }

    /// Open an in-memory cache (for testing)
    #[cfg(test)]
    pub fn open_in_memory(ttl_hours: u64) -> Result<Self> {
        Self::open_in_memory_with_max(ttl_hours, 1000)
    }

    #[cfg(test)]
    pub fn open_in_memory_with_max(ttl_hours: u64, max_entries: usize) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache (
                key TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                danger TEXT NOT NULL,
                explanation TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                command TEXT NOT NULL,
                exit_code INTEGER NOT NULL,
                danger TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
        )?;
        Ok(Self {
            conn,
            ttl_hours,
            max_entries,
        })
    }

    pub fn get(
        &self,
        query: &str,
        os: &str,
        shell: &str,
    ) -> Result<Option<(String, String, String)>> {
        let key = Self::make_key(query, os, shell);
        let now = now_secs();
        let ttl_secs = self.ttl_hours.saturating_mul(3600);

        let mut stmt = self.conn.prepare(
            "SELECT command, danger, explanation FROM cache WHERE key = ?1 AND (created_at + ?2) > ?3",
        )?;

        let result = stmt.query_row(rusqlite::params![key, ttl_secs, now], |row| {
            let cmd: String = row.get(0)?;
            let danger: String = row.get(1)?;
            let explanation: String = row.get(2)?;
            Ok((cmd, danger, explanation))
        });

        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn put(
        &self,
        query: &str,
        os: &str,
        shell: &str,
        command: &str,
        danger: &str,
        explanation: &str,
    ) -> Result<()> {
        let key = Self::make_key(query, os, shell);
        self.conn.execute(
            "INSERT OR REPLACE INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![key, command, danger, explanation, now_secs()],
        )?;
        if self.count()? > self.max_entries {
            self.evict_lru()?;
        }
        Ok(())
    }

    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    fn evict_expired(&self) -> Result<()> {
        let now = now_secs();
        let ttl_secs = self.ttl_hours.saturating_mul(3600);
        self.conn.execute(
            "DELETE FROM cache WHERE (created_at + ?1) <= ?2",
            rusqlite::params![ttl_secs, now],
        )?;
        Ok(())
    }

    fn evict_lru(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM cache WHERE key NOT IN (SELECT key FROM cache ORDER BY created_at DESC LIMIT ?1)",
            rusqlite::params![self.max_entries],
        )?;
        Ok(())
    }

    pub fn delete(&self, query: &str, os: &str, shell: &str) -> Result<()> {
        let key = Self::make_key(query, os, shell);
        self.conn
            .execute("DELETE FROM cache WHERE key = ?1", rusqlite::params![key])?;
        Ok(())
    }

    pub fn update_explanation(
        &self,
        query: &str,
        os: &str,
        shell: &str,
        explanation: &str,
    ) -> Result<()> {
        let key = Self::make_key(query, os, shell);
        self.conn.execute(
            "UPDATE cache SET explanation = ?1 WHERE key = ?2",
            rusqlite::params![explanation, key],
        )?;
        Ok(())
    }

    pub fn clear(&self) -> Result<u64> {
        let count = self.conn.execute("DELETE FROM cache", [])?;
        Ok(count as u64)
    }

    pub fn record_execution(
        &self,
        query: &str,
        command: &str,
        exit_code: i32,
        danger: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO history (query, command, exit_code, danger, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![query, command, exit_code, danger, now_secs()],
        )?;
        Ok(())
    }

    pub fn list_history(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT query, command, exit_code, danger, created_at FROM history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn search_history(&self, pattern: &str, limit: usize) -> Result<Vec<HistoryEntry>> {
        let like = format!("%{}%", pattern);
        let mut stmt = self.conn.prepare(
            "SELECT query, command, exit_code, danger, created_at FROM history WHERE query LIKE ?1 OR command LIKE ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![like, limit], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub(crate) fn make_key(query: &str, os: &str, shell: &str) -> String {
        let normalized = query.trim().to_lowercase();
        let input = format!("{}|{}|{}", normalized, os, shell);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_key_deterministic() {
        let k1 = Cache::make_key("list files", "Linux", "bash");
        let k2 = Cache::make_key("list files", "Linux", "bash");
        assert_eq!(k1, k2);
    }

    #[test]
    fn make_key_normalized_case() {
        let k1 = Cache::make_key("List Files", "Linux", "bash");
        let k2 = Cache::make_key("list files", "Linux", "bash");
        assert_eq!(k1, k2);
    }

    #[test]
    fn make_key_trimmed() {
        let k1 = Cache::make_key("  list files  ", "Linux", "bash");
        let k2 = Cache::make_key("list files", "Linux", "bash");
        assert_eq!(k1, k2);
    }

    #[test]
    fn make_key_differs_by_os() {
        let k1 = Cache::make_key("list files", "Linux", "bash");
        let k2 = Cache::make_key("list files", "Windows", "bash");
        assert_ne!(k1, k2);
    }

    #[test]
    fn make_key_differs_by_shell() {
        let k1 = Cache::make_key("list files", "Linux", "bash");
        let k2 = Cache::make_key("list files", "Linux", "zsh");
        assert_ne!(k1, k2);
    }

    #[test]
    fn make_key_differs_by_query() {
        let k1 = Cache::make_key("list files", "Linux", "bash");
        let k2 = Cache::make_key("show disk usage", "Linux", "bash");
        assert_ne!(k1, k2);
    }

    #[test]
    fn put_and_get_roundtrip() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .put("list files", "Linux", "bash", "ls -la", "safe", "")
            .unwrap();

        let result = cache.get("list files", "Linux", "bash").unwrap();
        assert_eq!(
            result,
            Some(("ls -la".to_string(), "safe".to_string(), "".to_string()))
        );
    }

    #[test]
    fn get_miss_returns_none() {
        let cache = Cache::open_in_memory(168).unwrap();
        let result = cache.get("nonexistent", "Linux", "bash").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn put_overwrites() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .put("q", "Linux", "bash", "old_cmd", "safe", "")
            .unwrap();
        cache
            .put("q", "Linux", "bash", "new_cmd", "warning", "")
            .unwrap();

        let (cmd, danger, _) = cache.get("q", "Linux", "bash").unwrap().unwrap();
        assert_eq!(cmd, "new_cmd");
        assert_eq!(danger, "warning");
    }

    #[test]
    fn clear_removes_all() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .put("q1", "Linux", "bash", "cmd1", "safe", "")
            .unwrap();
        cache
            .put("q2", "Linux", "bash", "cmd2", "safe", "")
            .unwrap();

        let count = cache.clear().unwrap();
        assert_eq!(count, 2);

        assert_eq!(cache.get("q1", "Linux", "bash").unwrap(), None);
        assert_eq!(cache.get("q2", "Linux", "bash").unwrap(), None);
    }

    #[test]
    fn expired_entry_not_returned() {
        // TTL = 0 hours means everything is expired immediately
        let cache = Cache::open_in_memory(0).unwrap();
        cache.put("q", "Linux", "bash", "ls", "safe", "").unwrap();

        // With TTL 0, created_at + 0 is not > now, so it should miss
        let result = cache.get("q", "Linux", "bash").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn record_and_list_history() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .record_execution("list files", "ls -la", 0, "safe")
            .unwrap();
        cache
            .record_execution("disk usage", "df -h", 0, "safe")
            .unwrap();
        let entries = cache.list_history(10).unwrap();
        assert_eq!(entries.len(), 2);
        // Both inserted, order by id DESC when same timestamp
    }

    #[test]
    fn search_history_by_pattern() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .record_execution("list files", "ls -la", 0, "safe")
            .unwrap();
        cache
            .record_execution("disk usage", "df -h", 0, "safe")
            .unwrap();
        let results = cache.search_history("files", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "list files");
    }

    #[test]
    fn delete_removes_entry() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .put("list files", "Linux", "bash", "ls -la", "safe", "")
            .unwrap();
        assert!(cache.get("list files", "Linux", "bash").unwrap().is_some());
        cache.delete("list files", "Linux", "bash").unwrap();
        assert!(cache.get("list files", "Linux", "bash").unwrap().is_none());
    }

    #[test]
    fn count_returns_correct() {
        let cache = Cache::open_in_memory(168).unwrap();
        assert_eq!(cache.count().unwrap(), 0);
        cache
            .put("q1", "Linux", "bash", "cmd1", "safe", "")
            .unwrap();
        assert_eq!(cache.count().unwrap(), 1);
        cache
            .put("q2", "Linux", "bash", "cmd2", "safe", "")
            .unwrap();
        assert_eq!(cache.count().unwrap(), 2);
    }

    #[test]
    fn evict_lru_keeps_newest() {
        let cache = Cache::open_in_memory_with_max(168, 2).unwrap();
        let now = now_secs();
        // Insert with explicit timestamps to ensure ordering
        let k1 = Cache::make_key("q1", "Linux", "bash");
        let k2 = Cache::make_key("q2", "Linux", "bash");
        let k3 = Cache::make_key("q3", "Linux", "bash");
        cache
            .conn
            .execute(
                "INSERT INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![k1, "cmd1", "safe", "", now - 20],
            )
            .unwrap();
        cache
            .conn
            .execute(
                "INSERT INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![k2, "cmd2", "safe", "", now - 10],
            )
            .unwrap();
        cache
            .conn
            .execute(
                "INSERT INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![k3, "cmd3", "safe", "", now],
            )
            .unwrap();
        assert_eq!(cache.count().unwrap(), 3);
        cache.evict_lru().unwrap();
        assert_eq!(cache.count().unwrap(), 2);
        // q1 (oldest) should be evicted, q2 and q3 should remain
        assert!(cache.get("q2", "Linux", "bash").unwrap().is_some());
        assert!(cache.get("q3", "Linux", "bash").unwrap().is_some());
    }

    #[test]
    fn evict_expired_removes_old() {
        // TTL = 0 means everything expired
        let cache = Cache::open_in_memory_with_max(0, 1000).unwrap();
        cache
            .conn
            .execute(
                "INSERT INTO cache (key, command, danger, explanation, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params!["old_key", "old_cmd", "safe", "", 0u64],
            )
            .unwrap();
        assert_eq!(cache.count().unwrap(), 1);
        cache.evict_expired().unwrap();
        assert_eq!(cache.count().unwrap(), 0);
    }

    #[test]
    fn put_and_get_with_explanation() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .put(
                "list files",
                "Linux",
                "bash",
                "ls -la",
                "safe",
                "ls: list dir\n-la: all + long",
            )
            .unwrap();

        let (cmd, danger, explanation) = cache.get("list files", "Linux", "bash").unwrap().unwrap();
        assert_eq!(cmd, "ls -la");
        assert_eq!(danger, "safe");
        assert_eq!(explanation, "ls: list dir\n-la: all + long");
    }

    #[test]
    fn put_empty_explanation_returns_empty() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache.put("q", "Linux", "bash", "cmd", "safe", "").unwrap();
        let (_, _, explanation) = cache.get("q", "Linux", "bash").unwrap().unwrap();
        assert_eq!(explanation, "");
    }

    #[test]
    fn update_explanation_existing_entry() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache.put("q", "Linux", "bash", "ls", "safe", "").unwrap();
        cache
            .update_explanation("q", "Linux", "bash", "ls: list files")
            .unwrap();
        let (_, _, explanation) = cache.get("q", "Linux", "bash").unwrap().unwrap();
        assert_eq!(explanation, "ls: list files");
    }

    #[test]
    fn update_explanation_nonexistent_noop() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .update_explanation("nope", "Linux", "bash", "text")
            .unwrap();
    }

    #[test]
    fn cross_platform_isolation() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .put("list files", "Linux", "bash", "ls -la", "safe", "")
            .unwrap();

        // Same query on Windows should not hit
        let result = cache.get("list files", "Windows", "PowerShell").unwrap();
        assert_eq!(result, None);
    }

    // ── Additional cache tests ──

    #[test]
    fn lru_eviction_boundary_exact_max() {
        let cache = Cache::open_in_memory_with_max(168, 3).unwrap();
        cache.put("q1", "L", "b", "c1", "safe", "").unwrap();
        cache.put("q2", "L", "b", "c2", "safe", "").unwrap();
        cache.put("q3", "L", "b", "c3", "safe", "").unwrap();
        assert_eq!(cache.count().unwrap(), 3);
        // 4th triggers eviction
        cache.put("q4", "L", "b", "c4", "safe", "").unwrap();
        assert!(cache.count().unwrap() <= 3);
    }

    #[test]
    fn history_ordering_newest_first() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache.record_execution("q1", "c1", 0, "safe").unwrap();
        cache.record_execution("q2", "c2", 0, "safe").unwrap();
        cache.record_execution("q3", "c3", 0, "safe").unwrap();
        let history = cache.list_history(10).unwrap();
        assert_eq!(history.len(), 3);
        // newest first (ORDER BY id DESC)
        assert_eq!(history[0].0, "q3");
        assert_eq!(history[2].0, "q1");
    }

    #[test]
    fn search_history_no_match() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .record_execution("git status", "git status", 0, "safe")
            .unwrap();
        let results = cache.search_history("nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_history_matches_command_field() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache
            .record_execution("list files", "ls -la", 0, "safe")
            .unwrap();
        // search should match both query and command
        let results = cache.search_history("ls", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn clear_does_not_affect_history() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache.put("q1", "L", "b", "c1", "safe", "").unwrap();
        cache.record_execution("q1", "c1", 0, "safe").unwrap();
        cache.clear().unwrap();
        assert_eq!(cache.count().unwrap(), 0);
        let history = cache.list_history(10).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn history_with_exit_code() {
        let cache = Cache::open_in_memory(168).unwrap();
        cache.record_execution("q", "cmd", 42, "warning").unwrap();
        let history = cache.list_history(10).unwrap();
        assert_eq!(history[0].2, 42); // exit_code
        assert_eq!(history[0].3, "warning"); // danger
    }

    #[test]
    fn list_history_respects_limit() {
        let cache = Cache::open_in_memory(168).unwrap();
        for i in 0..10 {
            cache
                .record_execution(&format!("q{}", i), &format!("c{}", i), 0, "safe")
                .unwrap();
        }
        let history = cache.list_history(3).unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn make_key_empty_query() {
        let k1 = Cache::make_key("", "Linux", "bash");
        let k2 = Cache::make_key("", "Linux", "bash");
        assert_eq!(k1, k2);
        // different from non-empty
        let k3 = Cache::make_key("x", "Linux", "bash");
        assert_ne!(k1, k3);
    }
}
