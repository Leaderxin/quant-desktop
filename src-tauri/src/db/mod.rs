use rusqlite::{Connection, Result as SqliteResult, params};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create database, auto-run migrations
    pub fn open(app_dir: PathBuf) -> SqliteResult<Self> {
        std::fs::create_dir_all(&app_dir).ok();
        let db_path = app_dir.join("quant-desktop.db");
        let conn = Connection::open(db_path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        db.init_defaults()?;
        Ok(db)
    }

    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watchlist (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                name        TEXT NOT NULL,
                sort_order  INTEGER DEFAULT 0,
                added_at    TEXT NOT NULL,
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

    /// Insert default settings values (only if key does not exist)
    pub fn init_defaults(&self) -> SqliteResult<()> {
        let defaults = [
            ("active_datasource", "eastmoney"),
            ("refresh_interval", "3"),
            ("theme", "dark"),
            ("ticker_visible", "true"),
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, code, market, name, sort_order, added_at
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
            })
        })?;
        rows.collect()
    }

    pub fn add_watch(&self, code: &str, market: &str, name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        conn.execute(
            "INSERT OR IGNORE INTO watchlist (code, market, name, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![code, market, name, now],
        )?;
        Ok(())
    }

    pub fn remove_watch(&self, code: &str, market: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM watchlist WHERE code = ?1 AND market = ?2",
            params![code, market],
        )?;
        Ok(())
    }

    pub fn reorder_watch(&self, ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        for (i, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE watchlist SET sort_order = ?1 WHERE id = ?2",
                params![i as i32, id],
            )?;
        }
        Ok(())
    }

    pub fn get_watch_codes(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(v)) => Ok(Some(v)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    // ── Quote Cache ──

    pub fn cache_quotes(&self, quotes: &[crate::domain::Quote]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        for q in quotes {
            let data = serde_json::to_string(q).unwrap_or_default();
            conn.execute(
                "INSERT OR REPLACE INTO quote_cache (code, market, data, cached_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![q.code, q.market, data, now],
            )?;
        }
        Ok(())
    }

    pub fn get_cached_quotes(&self) -> SqliteResult<Vec<crate::domain::Quote>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT data FROM quote_cache")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut quotes = Vec::new();
        for row in rows {
            if let Ok(data) = row {
                if let Ok(quote) = serde_json::from_str::<crate::domain::Quote>(&data) {
                    quotes.push(quote);
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
}
