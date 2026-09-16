use rusqlite::{Connection, Result as SqliteResult, params};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create database, auto-run migrations
    pub fn open(app_dir: PathBuf) -> SqliteResult<Self> {
        if let Err(e) = std::fs::create_dir_all(&app_dir) {
            log::warn!("Failed to create app data dir {:?}: {}", app_dir, e);
        }
        let db_path = app_dir.join("quant-desktop.db");
        let conn = Connection::open(db_path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        db.migrate_watchlist_codes()?;
        db.migrate_ticker_enabled()?;
        db.migrate_ticker_pinned()?;
        db.init_defaults()?;
        Ok(db)
    }

    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watchlist (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                name        TEXT NOT NULL,
                sort_order  INTEGER DEFAULT 0,
                added_at    TEXT NOT NULL,
                ticker_enabled INTEGER NOT NULL DEFAULT 1,
                UNIQUE(code, market)
            );
            CREATE TABLE IF NOT EXISTS settings (
                key         TEXT PRIMARY KEY,
                value       TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS quote_cache (
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                data        TEXT NOT NULL,
                cached_at   TEXT NOT NULL,
                PRIMARY KEY (code, market)
            );"
        )?;
        Ok(())
    }

    /// One-time data migration: prefix bare 6-digit CN watchlist codes with their
    /// exchange (`sh`/`sz`). Historical rows stored the bare code (e.g. "600519"),
    /// which is ambiguous when a code is shared between an index and a stock —
    /// e.g. 000852 is both sh000852 (中证1000 index) and sz000852 (石化机械 stock).
    /// The quote pipeline now keys on the full symbol, so legacy rows are upgraded.
    ///
    /// Idempotent: a full symbol already carries the sh/sz prefix (length 8) and is
    /// left untouched. The quote_cache is cleared because it holds serialized quotes
    /// keyed by the old bare code; it repopulates on the next poll.
    fn migrate_watchlist_codes(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("SELECT id, code FROM watchlist WHERE market = 'CN'")?;
        let rows: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<SqliteResult<_>>()?;
        drop(stmt);

        let mut migrated = false;
        for (id, code) in rows {
            if code.starts_with("sh") || code.starts_with("sz") {
                continue;
            }
            if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
                let prefix = if code.starts_with('6')
                    || code.starts_with('5')
                    || code.starts_with('9')
                {
                    "sh"
                } else {
                    "sz"
                };
                let full = format!("{}{}", prefix, code);
                conn.execute(
                    "UPDATE watchlist SET code = ?1 WHERE id = ?2",
                    params![full, id],
                )?;
                migrated = true;
            }
        }
        // Only clear the cache when legacy codes were actually rewritten. The
        // cache keys change from bare to full code, so stale entries are invalid;
        // but clearing it on every launch would defeat startup cache restoration.
        if migrated {
            conn.execute("DELETE FROM quote_cache", [])?;
        }
        Ok(())
    }

    /// 幂等迁移：为历史库补上 `ticker_enabled` 列。
    ///
    /// `CREATE TABLE IF NOT EXISTS` 对已存在的表不会加列，所以老库需要单独
    /// `ALTER TABLE`。SQLite 不支持 `ADD COLUMN IF NOT EXISTS`，重复执行会报
    /// "duplicate column name: ticker_enabled"，因此先用 PRAGMA 探测。
    ///
    /// 两点依赖的 SQLite 语义：
    /// - `ADD COLUMN` 是纯元数据操作，不重写表、不复制数据，任意规模均是 O(1)；
    ///   已有行读出时返回默认值，故历史自选全部默认开启。
    /// - `NOT NULL` 在 `ADD COLUMN` 上合法，前提是带非 NULL 默认值。
    fn migrate_ticker_enabled(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("PRAGMA table_info(watchlist)")?;
        let exists = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<SqliteResult<Vec<_>>>()?
            .iter()
            .any(|name| name == "ticker_enabled");
        drop(stmt);
        if !exists {
            conn.execute(
                "ALTER TABLE watchlist ADD COLUMN ticker_enabled INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
            log::info!("Migration: added watchlist.ticker_enabled");
        }
        Ok(())
    }

    fn migrate_ticker_pinned(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let columns = conn.prepare("PRAGMA table_info(watchlist)")?
            .query_map([], |row| row.get::<_, String>(1))?.collect::<SqliteResult<Vec<_>>>()?;
        if !columns.iter().any(|name| name == "ticker_pinned") {
            conn.execute("ALTER TABLE watchlist ADD COLUMN ticker_pinned INTEGER NOT NULL DEFAULT 0", [])?;
        }
        Ok(())
    }

    pub fn set_watch_ticker_pinned(&self, id: i64, pinned: bool) -> Result<(), String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let changed = conn.execute("UPDATE watchlist SET ticker_pinned = ?1 WHERE id = ?2", params![pinned, id])
            .map_err(|e| e.to_string())?;
        if changed == 0 { return Err("自选已不存在".into()); }
        Ok(())
    }

    /// Insert default settings values (default data source is Tencent)
    pub fn init_defaults(&self) -> SqliteResult<()> {
        let defaults = [
            ("active_datasource", "tencent"),
            ("refresh_interval", "3"),
            ("theme", "light"),
            ("ticker_visible", "1"),
            ("auto_launch", "false"),
        ];
        for (k, v) in defaults {
            if self.get_setting(k)?.is_none() {
                self.set_setting(k, v)?;
            }
        }
        Ok(())
    }

    // ── Watchlist CRUD ──

    pub fn get_watchlist(&self) -> SqliteResult<Vec<WatchItem>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, code, market, name, sort_order, added_at, ticker_enabled, ticker_pinned
             FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WatchItem {
                id: row.get(0)?,
                code: row.get(1)?,
                market: row.get(2)?,
                name: row.get(3)?,
                sort_order: row.get(4)?,
                added_at: row.get(5)?,
                ticker_enabled: row.get(6)?,
                ticker_pinned: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_watch(&self, code: &str, market: &str, name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        // Place new items at the end by computing the next sort_order from the
        // current maximum.  Without this, every new item would get DEFAULT 0
        // and appear at an unpredictable position after deletions leave gaps.
        let max_sort: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) FROM watchlist",
                [],
                |row| row.get(0),
            )
            .unwrap_or(-1);
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        conn.execute(
            "INSERT OR IGNORE INTO watchlist (code, market, name, sort_order, added_at, ticker_enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![code, market, name, max_sort + 1, now],
        )?;
        Ok(())
    }

    pub fn remove_watch(&self, code: &str, market: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "DELETE FROM watchlist WHERE code = ?1 AND market = ?2",
            params![code, market],
        )?;
        Ok(())
    }

    pub fn set_watch_ticker_enabled(&self, id: i64, enabled: bool) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "UPDATE watchlist SET ticker_enabled = ?1 WHERE id = ?2",
            params![enabled, id],
        )?;
        Ok(())
    }

    pub fn reorder_watch(&self, ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        // Wrap in a transaction so that a crash mid-reorder doesn't leave
        // sort_orders in an inconsistent half-updated state.
        let tx = conn.unchecked_transaction()?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![i as i32, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_watch_codes(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT code, market FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    // ── Settings CRUD ──

    pub fn get_setting(&self, key: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(v)) => Ok(Some(v)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    // ── Quote Cache ──

    pub fn cache_quotes(&self, quotes: &[crate::domain::Quote]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        // Wrap all INSERTs in a single transaction for better performance.
        // SAFETY: `unchecked_transaction` is safe here because:
        // - The connection is protected by a Mutex (no concurrent access).
        // - The loop below only executes INSERT/REPLACE (no reads that depend
        //   on uncommitted state within this transaction).
        // - If this function is refactored to remove the Mutex, replace with
        //   a regular `transaction()` to avoid data races.
        let tx = conn.unchecked_transaction()?;
        for q in quotes {
            let data = serde_json::to_string(q).unwrap_or_else(|e| {
                log::warn!(
                    "Failed to serialize quote {}:{} for cache: {}",
                    q.market, q.code, e
                );
                String::new()
            });
            if data.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO quote_cache (code, market, data, cached_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![q.code, q.market, data, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Atomic Watchlist Reorder Operations ──
    // Each method acquires the DB lock once and completes the entire
    // operation within that lock, preventing TOCTOU races.

    /// Move a watchlist entry to the top (sort_order = 0).
    /// All other entries are shifted down by one position.
    pub fn move_watch_top(&self, id: i64) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        let mut sort_order = 0i32;
        conn.execute(
            "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
            params![sort_order, id],
        )?;
        sort_order += 1;
        for other_id in &ids {
            if *other_id != id {
                conn.execute(
                    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                    params![sort_order, other_id],
                )?;
                sort_order += 1;
            }
        }
        Ok(())
    }

    /// Swap the target entry with the one above it (decrease sort_order).
    pub fn move_watch_up(&self, id: i64) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        if let Some(pos) = ids.iter().position(|&x| x == id) {
            if pos > 0 {
                let prev_id = ids[pos - 1];
                // Swap sort_order: target takes the previous entry's position,
                // and the previous entry takes the target's position.
                conn.execute(
                    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                    params![(pos - 1) as i32, id],
                )?;
                conn.execute(
                    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                    params![pos as i32, prev_id],
                )?;
            }
        }
        Ok(())
    }

    /// Swap the target entry with the one below it (increase sort_order).
    pub fn move_watch_down(&self, id: i64) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id FROM watchlist ORDER BY sort_order ASC, id ASC"
        )?;
        let ids: Vec<i64> = stmt.query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        if let Some(pos) = ids.iter().position(|&x| x == id) {
            if pos + 1 < ids.len() {
                let next_id = ids[pos + 1];
                conn.execute(
                    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                    params![(pos + 1) as i32, id],
                )?;
                conn.execute(
                    "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                    params![pos as i32, next_id],
                )?;
            }
        }
        Ok(())
    }

    pub fn get_cached_quotes(&self) -> SqliteResult<Vec<crate::domain::Quote>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("SELECT data FROM quote_cache")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut quotes = Vec::new();
        for row in rows {
            if let Ok(data) = row {
                match serde_json::from_str::<crate::domain::Quote>(&data) {
                    Ok(quote) => quotes.push(quote),
                    Err(e) => log::warn!(
                        "Failed to deserialize cached quote (skipping): {}",
                        e
                    ),
                }
            }
        }
        Ok(quotes)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchItem {
    pub id: i64,
    pub code: String,
    pub market: String,
    pub name: String,
    pub sort_order: i32,
    pub added_at: String,
    /// 是否参与行情条（ticker 窗口）滚动播报。新行默认 true。
    pub ticker_enabled: bool,
    pub ticker_pinned: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// 每个测试用独立的临时 app 目录，避免共享数据库文件互相干扰。
    /// 不加 `tempfile` 依赖，用进程 id + 纳秒 + 自增序号保证唯一。
    fn temp_app_dir(tag: &str) -> PathBuf {
        let seq = SEQ.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "qd-test-{}-{}-{}-{}",
            tag,
            std::process::id(),
            nanos,
            seq
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 写入一份旧版本 schema 的数据库（watchlist 无 ticker_enabled 列），
    /// 用于模拟「用户从旧版本升级上来」的路径。
    fn seed_legacy_db(dir: &Path) {
        let conn = Connection::open(dir.join("quant-desktop.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE watchlist (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                name        TEXT NOT NULL,
                sort_order  INTEGER DEFAULT 0,
                added_at    TEXT NOT NULL,
                UNIQUE(code, market)
            );
            INSERT INTO watchlist (code, market, name, sort_order, added_at)
            VALUES ('sh600519', 'CN', '贵州茅台', 0, '2026-01-01T00:00:00');",
        )
        .unwrap();
    }

    #[test]
    fn ticker_pin_migrates_and_survives_disable_reopen() {
        let dir = temp_app_dir("pin");
        seed_legacy_db(&dir);
        let db = Database::open(dir.clone()).unwrap();
        let item = db.get_watchlist().unwrap().remove(0);
        assert!(!item.ticker_pinned);
        db.set_watch_ticker_pinned(item.id, true).unwrap();
        db.set_watch_ticker_enabled(item.id, false).unwrap();
        drop(db);
        let db = Database::open(dir.clone()).unwrap();
        let item = db.get_watchlist().unwrap().remove(0);
        assert!(item.ticker_pinned);
        assert!(!item.ticker_enabled);
        db.set_watch_ticker_enabled(item.id, true).unwrap();
        assert!(db.get_watchlist().unwrap()[0].ticker_pinned);
        db.set_watch_ticker_pinned(item.id, false).unwrap();
        assert!(!db.get_watchlist().unwrap()[0].ticker_pinned);
        assert!(db.set_watch_ticker_pinned(-1, true).is_err());
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fresh_db_add_watch_defaults_ticker_enabled() {
        let dir = temp_app_dir("fresh");
        let db = Database::open(dir.clone()).unwrap();
        db.add_watch("sh600519", "CN", "贵州茅台").unwrap();

        let items = db.get_watchlist().unwrap();
        assert_eq!(items.len(), 1);
        assert!(
            items[0].ticker_enabled,
            "全新安装下新增自选应默认开启行情条播报"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn legacy_db_migrates_and_defaults_enabled() {
        let dir = temp_app_dir("legacy");
        seed_legacy_db(&dir);

        let db = Database::open(dir.clone()).unwrap();
        let items = db.get_watchlist().unwrap();
        assert_eq!(items.len(), 1, "迁移不应丢失历史自选");
        assert_eq!(items[0].name, "贵州茅台");
        assert_eq!(items[0].code, "sh600519");
        assert!(
            items[0].ticker_enabled,
            "历史自选迁移后应默认开启行情条播报"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn ticker_enabled_migration_is_idempotent() {
        let dir = temp_app_dir("idem");
        seed_legacy_db(&dir);

        // 第一次打开触发 ALTER TABLE
        {
            let _db = Database::open(dir.clone()).unwrap();
        }
        // 第二次打开列已存在，不应因 "duplicate column name" 报错
        let db = Database::open(dir.clone()).unwrap();
        db.add_watch("sz000001", "CN", "平安银行").unwrap();

        let items = db.get_watchlist().unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.ticker_enabled));

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn set_ticker_enabled_persists_across_reopen() {
        let dir = temp_app_dir("set");
        let id = {
            let db = Database::open(dir.clone()).unwrap();
            db.add_watch("sh600519", "CN", "贵州茅台").unwrap();
            let id = db.get_watchlist().unwrap()[0].id;
            db.set_watch_ticker_enabled(id, false).unwrap();
            assert!(!db.get_watchlist().unwrap()[0].ticker_enabled);
            id
        };

        // 重开确认关闭状态已落盘
        let db = Database::open(dir.clone()).unwrap();
        let items = db.get_watchlist().unwrap();
        assert_eq!(items[0].id, id);
        assert!(
            !items[0].ticker_enabled,
            "关闭状态应持久化，不应被迁移重置为开启"
        );

        // 重新开启，覆盖 `params![enabled, id]` 的另一个方向
        db.set_watch_ticker_enabled(id, true).unwrap();
        assert!(db.get_watchlist().unwrap()[0].ticker_enabled);
        drop(db);

        // 再次重开确认重新开启后的 true 也已落盘
        let db = Database::open(dir.clone()).unwrap();
        let items = db.get_watchlist().unwrap();
        assert_eq!(items[0].id, id);
        assert!(items[0].ticker_enabled, "重新开启后的状态应持久化");

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }
}
