use rusqlite::{Connection, Result as SqliteResult, params};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// 首个分组的名称。老用户升级后其全部自选落进这个分组。
pub const DEFAULT_GROUP_NAME: &str = "自选股";

/// 分组名称长度上限 —— 与前端输入框 maxlength 一致，避免超长名撑破分组标签栏。
pub const GROUP_NAME_MAX_LEN: usize = 20;

/// `settings` 表里所有键名的单一出处。
///
/// 为什么值得单列一层:这些键在 Rust 侧散落在启动恢复、窗口几何持久化、托盘开关、
/// 迁移标记等六七处,而且同一个键常被读写多次(`window_x` 三处、`ticker_x` 三处)。
/// 拼错一个字符串字面量不会有任何编译期报错 —— `get_setting` 返回 `None`,
/// 调用方按 `unwrap_or` 静默走兜底值;症状只是"窗口位置又没记住"这类模糊现象,
/// 追起来很费劲。
///
/// 值(如 `DEFAULT_INDEX_CODES_JSON`)不在这里,它们不是键。
pub mod keys {
    // ── 用户可见配置(设置页能改的) ──
    pub const ACTIVE_DATASOURCE: &str = "active_datasource";
    pub const THEME: &str = "theme";
    pub const AUTO_LAUNCH: &str = "auto_launch";
    pub const INDEX_CODES: &str = "index_codes";
    pub const MARKET_OVERVIEW_VISIBLE: &str = "market_overview_visible";
    pub const SECTOR_TOP_N: &str = "sector_top_n";
    pub const TICKER_VISIBLE: &str = "ticker_visible";
    pub const TICKER_TRANSPARENT: &str = "ticker_transparent";
    pub const TICKER_ITEMS_PER_PAGE: &str = "ticker_items_per_page";
    pub const WATCHLIST_COLUMNS: &str = "watchlist_columns";
    pub const WATCHLIST_DEFAULT_SORT: &str = "watchlist_default_sort";
    pub const COLOR_SCHEME: &str = "color_scheme";

    // ── 窗口几何:不在设置页里,由窗口 move/resize/close 事件自动持久化 ──
    pub const WINDOW_X: &str = "window_x";
    pub const WINDOW_Y: &str = "window_y";
    pub const WINDOW_WIDTH: &str = "window_width";
    pub const WINDOW_HEIGHT: &str = "window_height";
    pub const WINDOW_MAXIMIZED: &str = "window_maximized";

    /// 行情条窗口位置(用户拖动后保存,越界则回落右下角默认位)。
    pub const TICKER_X: &str = "ticker_x";
    pub const TICKER_Y: &str = "ticker_y";

    /// 一次性迁移的完成标记。内部键,不对用户暴露,因而不在 `DEFAULT_SETTINGS` 里:
    /// 它必须"不存在"才能触发迁移,给它写默认值会让迁移永不执行。
    pub const GROUPS_MIGRATED: &str = "groups_migrated";
}

/// 指数区默认勾选的指数(JSON 数组,顺序即顶栏展示顺序)。
/// 与 `crate::datasource::DEFAULT_INDEX_CODES` 是同一份数据的两种写法,
/// 由 `default_index_codes_match_datasource` 测试保证不会漂移。
const DEFAULT_INDEX_CODES_JSON: &str =
    r#"["s_sh000001","s_sz399001","s_sz399006","s_sh000688","s_sh000698","s_sh000905","s_sh000680"]"#;

/// 自选表默认展示的列(JSON 数组,顺序即列顺序)。
/// 不含 `ticker_enabled` —— 行情条播报开关已收归设置页,不再是表格列。
const DEFAULT_WATCHLIST_COLUMNS_JSON: &str =
    r#"["code","name","price","change_pct","change","volume","turnover","turnover_rate"]"#;

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
        db.migrate_watch_groups()?;
        db.init_defaults()?;
        Ok(db)
    }

    // ── Lock-free helpers ──
    //
    // 公开的 CRUD 方法各自 `lock()` 一次。任何**已在持锁状态下**执行的逻辑都必须
    // 走下面这几个接收 `&Connection` 的版本 —— `std::sync::Mutex` 不可重入，
    // 在持锁线程里再调 `self.get_setting()` 会直接死锁(不是报错，是挂住)。

    fn get_setting_conn(conn: &Connection, key: &str) -> SqliteResult<Option<String>> {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(Ok(v)) => Ok(Some(v)),
            _ => Ok(None),
        }
    }

    fn set_setting_conn(conn: &Connection, key: &str, value: &str) -> SqliteResult<()> {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// 某张表是否存在指定列。SQLite 没有 `ADD COLUMN IF NOT EXISTS`，重复 ADD 会
    /// 报 "duplicate column name"，所以每次升级都必须先探测。
    fn column_exists(conn: &Connection, table: &str, column: &str) -> SqliteResult<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let found = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<SqliteResult<Vec<_>>>()?
            .iter()
            .any(|c| c == column);
        Ok(found)
    }

    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute_batch(
            // watchlist 是**全局股票池**:一只股票只入池一次(UNIQUE(code, market)),
            // 分组归属走 watch_group_members 多对多表，因此池表本身不再带分组信息。
            //
            // `sort_order` 是 1.5.x 遗留的全局排序,多归属下同一只股票在不同分组
            // 位置不同，全局序号没有意义 —— 组内顺序已挪到关联表。老库该列仍在
            // (SQLite 删列要重建表)，但本文件不再读写它。
            "CREATE TABLE IF NOT EXISTS watchlist (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                code        TEXT NOT NULL,
                market      TEXT NOT NULL DEFAULT 'CN',
                name        TEXT NOT NULL,
                added_at    TEXT NOT NULL,
                ticker_enabled INTEGER NOT NULL DEFAULT 1,
                ticker_order INTEGER NOT NULL DEFAULT 0,
                UNIQUE(code, market)
            );
            CREATE TABLE IF NOT EXISTS watch_groups (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                sort_order  INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS watch_group_members (
                group_id    INTEGER NOT NULL,
                watch_id    INTEGER NOT NULL,
                sort_order  INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (group_id, watch_id)
            );
            CREATE INDEX IF NOT EXISTS idx_wgm_watch ON watch_group_members(watch_id);
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

    /// 幂等迁移:给 watchlist 补 `ticker_order` 列，保证「至少有一个分组」，
    /// 并把老库的全局自选顺序一次性搬进默认分组的关联表。
    ///
    /// 多归属模型:`watchlist` 是全局股票池,`watch_group_members` 是 (分组, 股票)
    /// 多对多关联,组内顺序存在关联表的 `sort_order` 上。同一只股票可以在多个分组
    /// 里各占一个不同位置 —— 这正是它不能沿用 watchlist 单一 sort_order 的原因。
    ///
    /// 幂等性来自四点:
    /// - `CREATE TABLE IF NOT EXISTS` 对已有库是空操作(在 `migrate` 里);
    /// - `ALTER TABLE` 先用 PRAGMA 探测列,避免 "duplicate column name";
    /// - 默认分组只在「表里一条都没有」时插入,不会每次启动追加一个「自选股」;
    /// - 关联表的灌入由 `groups_migrated` 设置键守住,只跑一次。
    ///   用设置键而不是「关联表为空」判断:用户完全可能合法地把所有股票移出所有分组
    ///   (此时池表仍有行)，按空表判断会导致下次启动被原样灌回。
    ///
    /// 老用户的观感:全部自选落进「自选股」且顺序不变;`ticker_order` 从遗留的
    /// `sort_order` 回填，所以行情条播报顺序也不会变。
    fn migrate_watch_groups(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        if !Self::column_exists(&conn, "watchlist", "ticker_order")? {
            conn.execute(
                "ALTER TABLE watchlist ADD COLUMN ticker_order INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
            log::info!("Migration: added watchlist.ticker_order");
        }
        // 老库(≤1.5.x)才有这一列;新装库没有，下面的回填要跳过。
        let has_legacy_sort = Self::column_exists(&conn, "watchlist", "sort_order")?;

        let group_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM watch_groups", [], |row| row.get(0))?;
        if group_count == 0 {
            let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
            conn.execute(
                "INSERT INTO watch_groups (name, sort_order, created_at) VALUES (?1, 0, ?2)",
                params![DEFAULT_GROUP_NAME, now],
            )?;
            log::info!("Migration: created default watch group '{}'", DEFAULT_GROUP_NAME);
        }

        if Self::get_setting_conn(&conn, keys::GROUPS_MIGRATED)?.is_some() {
            return Ok(());
        }

        let default_gid: i64 = conn.query_row(
            "SELECT id FROM watch_groups ORDER BY sort_order ASC, id ASC LIMIT 1",
            [],
            |row| row.get(0),
        )?;

        // 老库按遗留的全局 sort_order 还原顺序，新装库(空表)则无所谓。
        let order_clause = if has_legacy_sort {
            "ORDER BY sort_order ASC, id ASC"
        } else {
            "ORDER BY id ASC"
        };
        let watch_ids: Vec<i64> = {
            let mut stmt =
                conn.prepare(&format!("SELECT id FROM watchlist {}", order_clause))?;
            // 先绑定再 collect:直接把 collect 当块尾表达式的话，MappedRows 这个
            // 临时值会活到块尾，晚于 stmt 被 drop，触发 E0597。
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<SqliteResult<Vec<_>>>()?
        };

        let tx = conn.unchecked_transaction()?;
        for (seq, wid) in watch_ids.iter().enumerate() {
            tx.execute(
                "INSERT OR IGNORE INTO watch_group_members (group_id, watch_id, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![default_gid, wid, seq as i32],
            )?;
        }
        // 遗留全局顺序 → 行情条播报顺序。不回填的话所有行都是默认 0，
        // 老用户(尤其是调整过自选顺序的)会发现行情条轮播次序被打乱。
        if has_legacy_sort {
            tx.execute("UPDATE watchlist SET ticker_order = sort_order", [])?;
        }
        Self::set_setting_conn(&conn, keys::GROUPS_MIGRATED, "1")?;
        tx.commit()?;

        if !watch_ids.is_empty() {
            log::info!(
                "Migration: seeded {} watchlist row(s) into default group {}",
                watch_ids.len(),
                default_gid
            );
        }
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
        if !Self::column_exists(&conn, "watchlist", "ticker_enabled")? {
            conn.execute(
                "ALTER TABLE watchlist ADD COLUMN ticker_enabled INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
            log::info!("Migration: added watchlist.ticker_enabled");
        }
        Ok(())
    }

    /// 首启写入的默认设置。
    ///
    /// key 一律取自 `keys` —— 字符串字面量拼错时 `get_setting` 返回 `None`，
    /// 调用方按 `unwrap_or` 静默走兜底值，没有编译错误、没有日志、没有任何症状，
    /// 只是配置一直不生效。
    ///
    /// 值只在 key 不存在时写入，所以用户改过的设置不会被启动流程覆盖。
    /// 与前端 `src/stores/settings.ts` 的 `DEFAULTS` / `src/utils/prefs.ts` 的
    /// 兜底值对应，由 `default_settings_keys_are_pinned` 守住键名。
    pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
        (keys::ACTIVE_DATASOURCE, "tencent"),
        (keys::THEME, "light"),
        (keys::TICKER_VISIBLE, "1"),
        (keys::AUTO_LAUNCH, "false"),
        // 指数区:默认显示原有的 7 个,顺序即此处的顺序
        (keys::INDEX_CODES, DEFAULT_INDEX_CODES_JSON),
        // 市场概览
        (keys::MARKET_OVERVIEW_VISIBLE, "1"),
        (keys::SECTOR_TOP_N, "5"),
        // 行情条
        (keys::TICKER_TRANSPARENT, "0"),
        (keys::TICKER_ITEMS_PER_PAGE, "2"),
        // 自选列表
        (keys::WATCHLIST_COLUMNS, DEFAULT_WATCHLIST_COLUMNS_JSON),
        (keys::WATCHLIST_DEFAULT_SORT, ""),
        (keys::COLOR_SCHEME, "cn"),
    ];

    /// Insert default settings values (default data source is Tencent)
    pub fn init_defaults(&self) -> SqliteResult<()> {
        for (k, v) in Self::DEFAULT_SETTINGS {
            if self.get_setting(k)?.is_none() {
                self.set_setting(k, v)?;
            }
        }
        Ok(())
    }

    // ── Watchlist pool ──

    /// 全局股票池。分组归属不在这个结构里 —— 见 `get_watchlist_snapshot`。
    pub fn get_watchlist(&self) -> SqliteResult<Vec<WatchItem>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, code, market, name, added_at, ticker_enabled, ticker_order
             FROM watchlist ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WatchItem {
                id: row.get(0)?,
                code: row.get(1)?,
                market: row.get(2)?,
                name: row.get(3)?,
                added_at: row.get(4)?,
                ticker_enabled: row.get(5)?,
                ticker_order: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// 一次 IPC 返回自选表需要的全部状态:股票池 + 分组(含组内有序成员 id) + 默认分组。
    ///
    /// 合并成一个命令而不是两个:分组标签栏切换必须**瞬时**,如果分组与成员分开拉,
    /// 切分组时要么再往返一次、要么在本地拼接出中间态。一次拿全，前端切换分组就是
    /// 纯本地过滤。
    pub fn get_watchlist_snapshot(&self) -> SqliteResult<WatchlistSnapshot> {
        let items = self.get_watchlist()?;
        let mut groups = self.get_watch_groups()?;

        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut members: HashMap<i64, Vec<i64>> = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT group_id, watch_id FROM watch_group_members
                 ORDER BY group_id ASC, sort_order ASC, watch_id ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (gid, wid) = row?;
                members.entry(gid).or_default().push(wid);
            }
        }
        for g in &mut groups {
            g.watch_ids = members.remove(&g.id).unwrap_or_default();
        }

        let default_group_id = groups.first().map(|g| g.id).unwrap_or(0);
        Ok(WatchlistSnapshot {
            items,
            groups,
            default_group_id,
        })
    }

    /// 下一个可用的行情条播报位次(追加到队尾)。
    fn next_ticker_order(conn: &Connection) -> SqliteResult<i32> {
        conn.query_row(
            "SELECT COALESCE(MAX(ticker_order), -1) + 1 FROM watchlist",
            [],
            |row| row.get(0),
        )
    }

    /// 全部现存分组 id。用来挡住「往不存在的分组里加股票」。
    fn all_group_ids(conn: &Connection) -> SqliteResult<std::collections::HashSet<i64>> {
        let mut stmt = conn.prepare("SELECT id FROM watch_groups")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<SqliteResult<std::collections::HashSet<_>>>()
    }

    /// 默认分组 id —— 展示顺序最靠前的那个。迁移保证了它一定存在。
    fn default_group_id(conn: &Connection) -> SqliteResult<i64> {
        conn.query_row(
            "SELECT id FROM watch_groups ORDER BY sort_order ASC, id ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
    }

    /// 把股票加进池子(已在池中则复用),并加入指定分组。
    ///
    /// 多归属语义:已在其它分组也能直接加入本组,不做任何移动。重复加入同一分组
    /// 是空操作(`INSERT OR IGNORE`),不会把它从原位置挪到队尾。
    ///
    /// `group_id` 不存在时落到默认分组,而不是照单写入。SQLite 默认不启用外键约束,
    /// 写进去就是一条指向虚空分组的关联 —— 那只股票在池子里、却不在任何分组里，
    /// 界面上没有任何入口能再看到或删掉它。
    pub fn add_watch(
        &self,
        code: &str,
        market: &str,
        name: &str,
        group_id: i64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let gid = if Self::all_group_ids(&conn)?.contains(&group_id) {
            group_id
        } else {
            log::warn!(
                "add_watch: group {} does not exist, falling back to the default group",
                group_id
            );
            Self::default_group_id(&conn)?
        };
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO watchlist
                (code, market, name, added_at, ticker_enabled, ticker_order)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)
             ON CONFLICT(code, market) DO NOTHING",
            params![code, market, name, now, Self::next_ticker_order(&conn)?],
        )?;
        let watch_id: i64 = tx.query_row(
            "SELECT id FROM watchlist WHERE code = ?1 AND market = ?2",
            params![code, market],
            |row| row.get(0),
        )?;
        let next_sort: i32 = tx.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM watch_group_members WHERE group_id = ?1",
            params![gid],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO watch_group_members (group_id, watch_id, sort_order)
             VALUES (?1, ?2, ?3)",
            params![gid, watch_id, next_sort],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 彻底删除自选:解除全部分组关联并从池中移除。
    pub fn remove_watch(&self, code: &str, market: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM watch_group_members WHERE watch_id IN
                (SELECT id FROM watchlist WHERE code = ?1 AND market = ?2)",
            params![code, market],
        )?;
        tx.execute(
            "DELETE FROM watchlist WHERE code = ?1 AND market = ?2",
            params![code, market],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// 把一只自选移出某个分组。
    ///
    /// 若它因此在**所有**分组里都不存在了，就顺手从池中删掉并返回 true ——
    /// 留一个不属于任何分组的股票会让它在界面上无处可见，等于数据还在但用户
    /// 找不回来。返回 true 时前端的菜单文案本来就是「删除自选」而非「从本组移除」，
    /// 所以这个行为对用户是可预期的。
    pub fn remove_watch_from_group(&self, group_id: i64, watch_id: i64) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM watch_group_members WHERE group_id = ?1 AND watch_id = ?2",
            params![group_id, watch_id],
        )?;
        let remaining: i64 = tx.query_row(
            "SELECT COUNT(*) FROM watch_group_members WHERE watch_id = ?1",
            params![watch_id],
            |row| row.get(0),
        )?;
        if remaining == 0 {
            tx.execute("DELETE FROM watchlist WHERE id = ?1", params![watch_id])?;
        }
        tx.commit()?;
        Ok(remaining == 0)
    }

    /// 全量覆盖一只自选的分组归属(右键「添加到分组」多选提交)。
    ///
    /// 空集合会被拒绝而不是解释成「删除」:勾选框全清空是个很容易误触的状态，
    /// 静默删数据代价太大。真要删走「删除自选」。
    ///
    /// 不存在的分组 id 会被剔除 —— 菜单是打开时生成的快照,期间分组可能已被删除。
    /// 剔除后若一个都不剩,按空集合处理(同样的拒绝,而不是写出一只隐形股票)。
    pub fn set_watch_groups(&self, watch_id: i64, group_ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let alive = Self::all_group_ids(&conn)?;
        let group_ids: Vec<i64> = group_ids
            .iter()
            .copied()
            .filter(|id| alive.contains(id))
            .collect();

        if group_ids.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "至少要属于一个分组，如需删除请用「删除自选」".into(),
            ));
        }
        let tx = conn.unchecked_transaction()?;

        // 先删掉不在目标集合里的关联。删完再补，避免「先补后删」把刚加上的又抹掉。
        let placeholders = group_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        {
            let mut sql = format!(
                "DELETE FROM watch_group_members WHERE watch_id = ? AND group_id NOT IN ({})",
                placeholders
            );
            let mut args: Vec<&dyn rusqlite::ToSql> = vec![&watch_id];
            for g in &group_ids {
                args.push(g);
            }
            tx.execute(&mut sql, args.as_slice())?;
        }

        for gid in &group_ids {
            // 已在该组则 IGNORE，保持其原有组内位置不变
            let next_sort: i32 = tx.query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM watch_group_members WHERE group_id = ?1",
                params![gid],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO watch_group_members (group_id, watch_id, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![gid, watch_id, next_sort],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Ticker (行情条播报范围) ──
    //
    // 播报范围是**扁平列表**，与分组正交:一只股票在几个分组里都只占一个播报位。
    // 所以 `ticker_order` 挂在池表上，而不是关联表。

    /// 切换单只自选的行情条播报开关。
    /// 开启时把它排到播报队列末尾 —— 默认落 0 会插到队首,与「刚加进来的排最后」
    /// 的直觉相反,也会把用户已有的播报顺序顶开。
    pub fn set_watch_ticker_enabled(&self, id: i64, enabled: bool) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        if enabled {
            conn.execute(
                "UPDATE watchlist SET ticker_enabled = 1, ticker_order = ?1 WHERE id = ?2",
                params![Self::next_ticker_order(&conn)?, id],
            )?;
        } else {
            conn.execute(
                "UPDATE watchlist SET ticker_enabled = 0 WHERE id = ?1",
                params![id],
            )?;
        }
        Ok(())
    }

    /// 批量开关播报(设置页按分组整体勾选/取消)。开启时按传入顺序追加到队尾。
    pub fn set_ticker_enabled_bulk(&self, ids: &[i64], enabled: bool) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.unchecked_transaction()?;
        if enabled {
            let mut order = Self::next_ticker_order(&conn)?;
            for id in ids {
                tx.execute(
                    "UPDATE watchlist SET ticker_enabled = 1, ticker_order = ?1 WHERE id = ?2",
                    params![order, id],
                )?;
                order += 1;
            }
        } else {
            for id in ids {
                tx.execute(
                    "UPDATE watchlist SET ticker_enabled = 0 WHERE id = ?1",
                    params![id],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 重排行情条播报顺序。传入的是设置页里播报范围列表的顺序。
    pub fn reorder_ticker(&self, ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.unchecked_transaction()?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE watchlist SET ticker_order = ?1 WHERE id = ?2",
                params![i as i32, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── 组内排序 ──

    fn group_member_ids(conn: &Connection, group_id: i64) -> SqliteResult<Vec<i64>> {
        let mut stmt = conn.prepare(
            "SELECT watch_id FROM watch_group_members WHERE group_id = ?1
             ORDER BY sort_order ASC, watch_id ASC",
        )?;
        let rows = stmt.query_map(params![group_id], |row| row.get(0))?;
        rows.collect::<SqliteResult<Vec<_>>>()
    }

    /// 重排某个分组内的自选顺序。
    pub fn reorder_group_members(&self, group_id: i64, watch_ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.unchecked_transaction()?;
        for (i, wid) in watch_ids.iter().enumerate() {
            tx.execute(
                "UPDATE watch_group_members SET sort_order = ?1
                 WHERE group_id = ?2 AND watch_id = ?3",
                params![i as i32, group_id, wid],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// 组内上移/下移/置顶 —— 三者共用一个实现。
    ///
    /// 必须在**组内**重排:沿用全局自选顺序的话，上移会让股票跨过分组边界跑到
    /// 别的分组的前面去(单归属时代 sort_order 是全局的，这个坑更隐蔽)。
    fn move_group_member(&self, group_id: i64, watch_id: i64, mode: MoveMode) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut ids = Self::group_member_ids(&conn, group_id)?;
        let Some(pos) = ids.iter().position(|&x| x == watch_id) else {
            return Ok(());
        };
        match mode {
            MoveMode::Top => {
                ids.remove(pos);
                ids.insert(0, watch_id);
            }
            MoveMode::Up => {
                if pos == 0 {
                    return Ok(());
                }
                ids.swap(pos - 1, pos);
            }
            MoveMode::Down => {
                if pos + 1 >= ids.len() {
                    return Ok(());
                }
                ids.swap(pos, pos + 1);
            }
        }
        let tx = conn.unchecked_transaction()?;
        for (i, wid) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE watch_group_members SET sort_order = ?1
                 WHERE group_id = ?2 AND watch_id = ?3",
                params![i as i32, group_id, wid],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn move_group_member_top(&self, group_id: i64, watch_id: i64) -> SqliteResult<()> {
        self.move_group_member(group_id, watch_id, MoveMode::Top)
    }
    pub fn move_group_member_up(&self, group_id: i64, watch_id: i64) -> SqliteResult<()> {
        self.move_group_member(group_id, watch_id, MoveMode::Up)
    }
    pub fn move_group_member_down(&self, group_id: i64, watch_id: i64) -> SqliteResult<()> {
        self.move_group_member(group_id, watch_id, MoveMode::Down)
    }

    /// 调度器用:池中全部 (code, market)。顺序对批量请求没有影响。
    pub fn get_watch_codes(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare("SELECT code, market FROM watchlist ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    // ── Watch Groups ──

    pub fn get_watch_groups(&self) -> SqliteResult<Vec<WatchGroup>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, name, sort_order, created_at FROM watch_groups
             ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WatchGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
                created_at: row.get(3)?,
                watch_ids: Vec::new(),
            })
        })?;
        rows.collect()
    }

    fn validate_group_name(name: &str) -> SqliteResult<&str> {
        let name = name.trim();
        if name.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName("分组名不能为空".into()));
        }
        if name.chars().count() > GROUP_NAME_MAX_LEN {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "分组名不能超过 {} 个字符",
                GROUP_NAME_MAX_LEN
            )));
        }
        Ok(name)
    }

    /// 新建分组,追加到末尾。返回完整分组行,前端无需再拉一次列表。
    pub fn add_watch_group(&self, name: &str) -> SqliteResult<WatchGroup> {
        let name = Self::validate_group_name(name)?.to_string();
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let next: i32 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM watch_groups",
            [],
            |row| row.get(0),
        )?;
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        conn.execute(
            "INSERT INTO watch_groups (name, sort_order, created_at) VALUES (?1, ?2, ?3)",
            params![name, next, now],
        )?;
        Ok(WatchGroup {
            id: conn.last_insert_rowid(),
            name,
            sort_order: next,
            created_at: now,
            watch_ids: Vec::new(),
        })
    }

    pub fn rename_watch_group(&self, id: i64, name: &str) -> SqliteResult<()> {
        let name = Self::validate_group_name(name)?.to_string();
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let clash = conn
            .query_row(
                "SELECT 1 FROM watch_groups WHERE name = ?1 AND id != ?2",
                params![name, id],
                |_| Ok(()),
            )
            .is_ok();
        if clash {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "已存在名为「{}」的分组",
                name
            )));
        }
        conn.execute(
            "UPDATE watch_groups SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(())
    }

    /// 删除分组。**不删自选**,只解除该分组的关联;
    /// 因此变成「不属于任何分组」的股票会被并入默认分组(展示顺序最靠前的那个)。
    ///
    /// 为什么不是「连同自选一起删」:自选是用户积累的资产,删分组通常是想整理标签
    /// 而不是丢数据。为什么只救孤儿而不是把整组都并过去:还留在其它分组里的股票
    /// 本来就有地方可去，多塞一个「自选股」反而是多余的噪音。
    ///
    /// 返回被救回的孤儿数量，供 UI 在确认/提示里说明影响面。
    /// 最后一个分组不允许删 —— 没有分组可归属的话整个自选表就无处安放了。
    pub fn delete_watch_group(&self, id: i64) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let total: i64 =
            conn.query_row("SELECT COUNT(*) FROM watch_groups", [], |row| row.get(0))?;
        if total <= 1 {
            return Err(rusqlite::Error::InvalidParameterName(
                "至少需要保留一个分组".into(),
            ));
        }
        let fallback: i64 = conn.query_row(
            "SELECT id FROM watch_groups WHERE id != ?1 ORDER BY sort_order ASC, id ASC LIMIT 1",
            params![id],
            |row| row.get(0),
        )?;

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM watch_group_members WHERE group_id = ?1",
            params![id],
        )?;
        // 关联删完后的孤儿 = 池中存在、但关联表里已无任何行的股票
        let orphans: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT w.id FROM watchlist w
                 WHERE NOT EXISTS (
                     SELECT 1 FROM watch_group_members m WHERE m.watch_id = w.id
                 )
                 ORDER BY w.id ASC",
            )?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<SqliteResult<Vec<_>>>()?
        };
        let mut next_sort: i32 = tx.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM watch_group_members WHERE group_id = ?1",
            params![fallback],
            |row| row.get(0),
        )?;
        for wid in &orphans {
            tx.execute(
                "INSERT OR IGNORE INTO watch_group_members (group_id, watch_id, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![fallback, wid, next_sort],
            )?;
            next_sort += 1;
        }
        tx.execute("DELETE FROM watch_groups WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(orphans.len())
    }

    pub fn reorder_watch_groups(&self, ids: &[i64]) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.unchecked_transaction()?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE watch_groups SET sort_order = ?1 WHERE id = ?2",
                params![i as i32, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ── Settings CRUD ──

    pub fn get_setting(&self, key: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        Self::get_setting_conn(&conn, key)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        Self::set_setting_conn(&conn, key, value)
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

#[derive(Debug, Clone, Copy)]
enum MoveMode {
    Top,
    Up,
    Down,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchItem {
    pub id: i64,
    pub code: String,
    pub market: String,
    pub name: String,
    pub added_at: String,
    /// 是否参与行情条（ticker 窗口）滚动播报。新行默认 true。
    pub ticker_enabled: bool,
    /// 行情条轮播位次。与分组无关 —— 播报范围是跨分组的扁平列表。
    pub ticker_order: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchGroup {
    pub id: i64,
    pub name: String,
    pub sort_order: i32,
    pub created_at: String,
    /// 组内成员的自选 id,按组内顺序。由 `get_watchlist_snapshot` 填充;
    /// `get_watch_groups` 单独调用时为空数组。
    pub watch_ids: Vec<i64>,
}

/// 自选表一次拉全的状态。分组标签栏切换因此是纯本地过滤，无需再往返 IPC。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchlistSnapshot {
    pub items: Vec<WatchItem>,
    pub groups: Vec<WatchGroup>,
    pub default_group_id: i64,
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

    /// 写入一份 1.5.x schema 的数据库(watchlist 无 ticker_enabled / ticker_order，
    /// 且带遗留的全局 sort_order)，用于模拟「用户从旧版本升级上来」的路径。
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
            VALUES ('sh600519', 'CN', '贵州茅台', 1, '2026-01-01T00:00:00'),
                   ('sz000001', 'CN', '平安银行', 0, '2026-01-01T00:00:00');",
        )
        .unwrap();
    }

    /// 便捷断言：某只自选当前所属的分组名集合(已排序)。
    fn group_names_of(db: &Database, watch_id: i64) -> Vec<String> {
        let snap = db.get_watchlist_snapshot().unwrap();
        let mut names: Vec<String> = snap
            .groups
            .iter()
            .filter(|g| g.watch_ids.contains(&watch_id))
            .map(|g| g.name.clone())
            .collect();
        names.sort();
        names
    }

    fn watch_id_of(db: &Database, code: &str) -> i64 {
        db.get_watchlist()
            .unwrap()
            .into_iter()
            .find(|w| w.code == code)
            .unwrap_or_else(|| panic!("watchlist should contain {}", code))
            .id
    }

    // ── 迁移 ──

    #[test]
    fn fresh_db_add_watch_defaults_ticker_enabled() {
        let dir = temp_app_dir("fresh");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        db.add_watch("sh600519", "CN", "贵州茅台", gid).unwrap();

        let items = db.get_watchlist().unwrap();
        assert_eq!(items.len(), 1);
        assert!(
            items[0].ticker_enabled,
            "全新安装下新增自选应默认开启行情条播报"
        );
        assert_eq!(
            group_names_of(&db, items[0].id),
            vec![DEFAULT_GROUP_NAME.to_string()],
            "新增自选应落在默认分组"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn legacy_db_migrates_into_default_group_preserving_order() {
        let dir = temp_app_dir("legacy");
        seed_legacy_db(&dir);

        let db = Database::open(dir.clone()).unwrap();
        let snap = db.get_watchlist_snapshot().unwrap();
        assert_eq!(snap.items.len(), 2, "迁移不应丢失历史自选");
        assert_eq!(snap.groups.len(), 1);
        let g = &snap.groups[0];
        assert_eq!(g.name, DEFAULT_GROUP_NAME);
        assert_eq!(g.watch_ids.len(), 2, "历史自选应全部落进默认分组");

        // 遗留 sort_order 为 平安银行(0) < 贵州茅台(1)，组内顺序必须照搬
        let pingan = watch_id_of(&db, "sz000001");
        let maotai = watch_id_of(&db, "sh600519");
        assert_eq!(
            g.watch_ids,
            vec![pingan, maotai],
            "组内顺序应沿用历史的全局 sort_order"
        );
        assert!(snap.items.iter().all(|i| i.ticker_enabled));

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn legacy_db_seeds_ticker_order_from_sort_order() {
        let dir = temp_app_dir("ticker-order");
        seed_legacy_db(&dir);

        let db = Database::open(dir.clone()).unwrap();
        let pingan = db
            .get_watchlist()
            .unwrap()
            .into_iter()
            .find(|w| w.code == "sz000001")
            .unwrap();
        let maotai = db
            .get_watchlist()
            .unwrap()
            .into_iter()
            .find(|w| w.code == "sh600519")
            .unwrap();
        assert_eq!(pingan.ticker_order, 0);
        assert_eq!(
            maotai.ticker_order, 1,
            "行情条播报顺序应从遗留的全局 sort_order 回填，否则老用户轮播次序会被打乱"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = temp_app_dir("idem");
        seed_legacy_db(&dir);

        // 第一次打开触发 ALTER TABLE + 分组灌入
        {
            let _db = Database::open(dir.clone()).unwrap();
        }
        // 第二次打开列与分组均已存在，不应报 duplicate column / 多出一个「自选股」
        let db = Database::open(dir.clone()).unwrap();
        let snap = db.get_watchlist_snapshot().unwrap();
        assert_eq!(snap.groups.len(), 1, "重复启动不应追加默认分组");
        assert_eq!(snap.groups[0].watch_ids.len(), 2);
        assert_eq!(db.get_watchlist().unwrap().len(), 2);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    /// 用户把所有股票移出所有分组后，池表仍有行；此时重启不应把关联灌回来。
    /// 这正是用 `groups_migrated` 设置键而不是「关联表为空」判断的原因。
    #[test]
    fn emptied_memberships_are_not_reseeded_on_restart() {
        let dir = temp_app_dir("reseed");
        let gid = {
            let db = Database::open(dir.clone()).unwrap();
            let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
            db.add_watch("sh600519", "CN", "贵州茅台", gid).unwrap();
            let wid = watch_id_of(&db, "sh600519");
            // 移出唯一分组 => 整只股票被删除
            assert!(db.remove_watch_from_group(gid, wid).unwrap());
            assert!(db.get_watchlist().unwrap().is_empty());
            gid
        };

        let db = Database::open(dir.clone()).unwrap();
        let snap = db.get_watchlist_snapshot().unwrap();
        assert!(snap.items.is_empty(), "重启不应把已清空的自选灌回来");
        assert!(snap.groups[0].watch_ids.is_empty());
        assert_eq!(snap.groups[0].id, gid);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    // ── 行情条播报开关 ──

    #[test]
    fn set_ticker_enabled_persists_across_reopen() {
        let dir = temp_app_dir("set");
        let id = {
            let db = Database::open(dir.clone()).unwrap();
            let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
            db.add_watch("sh600519", "CN", "贵州茅台", gid).unwrap();
            let id = watch_id_of(&db, "sh600519");
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

    #[test]
    fn enabling_ticker_appends_to_queue_tail() {
        let dir = temp_app_dir("tail");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        for (code, name) in [("sh600519", "贵州茅台"), ("sz000001", "平安银行")] {
            db.add_watch(code, "CN", name, gid).unwrap();
        }
        let first = watch_id_of(&db, "sh600519");
        let second = watch_id_of(&db, "sz000001");

        db.set_watch_ticker_enabled(first, false).unwrap();
        db.set_watch_ticker_enabled(first, true).unwrap();

        let first_order = db
            .get_watchlist()
            .unwrap()
            .into_iter()
            .find(|w| w.id == first)
            .unwrap()
            .ticker_order;
        let second_order = db
            .get_watchlist()
            .unwrap()
            .into_iter()
            .find(|w| w.id == second)
            .unwrap()
            .ticker_order;
        assert!(
            first_order > second_order,
            "重新开启播报应排到队尾（{} 应大于 {}），而不是插回队首",
            first_order,
            second_order
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn bulk_ticker_enable_assigns_sequential_order() {
        let dir = temp_app_dir("bulk");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        for code in ["sh600519", "sz000001", "sz300750"] {
            db.add_watch(code, "CN", code, gid).unwrap();
        }
        let ids = vec![
            watch_id_of(&db, "sz300750"),
            watch_id_of(&db, "sh600519"),
            watch_id_of(&db, "sz000001"),
        ];
        db.set_ticker_enabled_bulk(&ids, false).unwrap();
        db.set_ticker_enabled_bulk(&ids, true).unwrap();

        let items = db.get_watchlist().unwrap();
        let order = |code: &str| items.iter().find(|w| w.code == code).unwrap().ticker_order;
        assert!(
            order("sz300750") < order("sh600519") && order("sh600519") < order("sz000001"),
            "批量开启应按传入顺序依次追加"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    // ── 多归属 ──

    #[test]
    fn same_stock_can_belong_to_multiple_groups() {
        let dir = temp_app_dir("multi");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        let wid = watch_id_of(&db, "sz300750");
        db.set_watch_groups(wid, &[g1, g2]).unwrap();

        assert_eq!(
            group_names_of(&db, wid),
            vec!["科技".to_string(), DEFAULT_GROUP_NAME.to_string()]
        );
        assert_eq!(db.get_watchlist().unwrap().len(), 1, "多归属不应产生重复池行");

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn adding_to_another_group_does_not_move_it() {
        let dir = temp_app_dir("nomove");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        // 再加一次到另一个分组:多归属下应当"两边都在"，而不是搬走
        db.add_watch("sz300750", "CN", "宁德时代", g2).unwrap();

        let wid = watch_id_of(&db, "sz300750");
        assert_eq!(group_names_of(&db, wid).len(), 2);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn adding_twice_to_same_group_keeps_position() {
        let dir = temp_app_dir("dup");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        for code in ["sh600519", "sz000001"] {
            db.add_watch(code, "CN", code, gid).unwrap();
        }
        db.move_group_member_down(gid, watch_id_of(&db, "sh600519"))
            .unwrap();
        let before = db.get_watchlist_snapshot().unwrap().groups[0].watch_ids.clone();

        db.add_watch("sh600519", "CN", "贵州茅台", gid).unwrap();
        let after = db.get_watchlist_snapshot().unwrap().groups[0].watch_ids.clone();
        assert_eq!(after, before, "重复加入同一分组不应把它挪到队尾");

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn removing_from_one_group_keeps_it_in_others() {
        let dir = temp_app_dir("partial");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        let wid = watch_id_of(&db, "sz300750");
        db.set_watch_groups(wid, &[g1, g2]).unwrap();

        let deleted = db.remove_watch_from_group(g2, wid).unwrap();
        assert!(!deleted, "还有其它分组时不应删除股票本体");
        assert_eq!(group_names_of(&db, wid), vec![DEFAULT_GROUP_NAME.to_string()]);
        assert_eq!(db.get_watchlist().unwrap().len(), 1);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn removing_from_last_group_deletes_the_stock() {
        let dir = temp_app_dir("orphan");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        db.add_watch("sz300750", "CN", "宁德时代", gid).unwrap();
        let wid = watch_id_of(&db, "sz300750");

        let deleted = db.remove_watch_from_group(gid, wid).unwrap();
        assert!(deleted, "移出最后一个分组时应当连股票本体一起删掉");
        assert!(
            db.get_watchlist().unwrap().is_empty(),
            "不应留下不属于任何分组的孤儿行"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn set_watch_groups_rejects_empty_selection() {
        let dir = temp_app_dir("empty-set");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        db.add_watch("sz300750", "CN", "宁德时代", gid).unwrap();
        let wid = watch_id_of(&db, "sz300750");

        assert!(
            db.set_watch_groups(wid, &[]).is_err(),
            "清空全部分组应被拒绝，而不是静默删除股票"
        );
        assert_eq!(db.get_watchlist().unwrap().len(), 1);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn set_watch_groups_removes_membership_not_in_list() {
        let dir = temp_app_dir("overwrite");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        let wid = watch_id_of(&db, "sz300750");
        db.set_watch_groups(wid, &[g1, g2]).unwrap();

        // 全量覆盖为只剩「科技」
        db.set_watch_groups(wid, &[g2]).unwrap();
        assert_eq!(group_names_of(&db, wid), vec!["科技".to_string()]);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    // ── 分组 CRUD ──

    #[test]
    fn delete_group_moves_only_orphans_to_default() {
        let dir = temp_app_dir("delgroup");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sh600519", "CN", "贵州茅台", g2).unwrap(); // 仅科技
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap(); // 自选股 + 科技
        let both = watch_id_of(&db, "sz300750");
        db.set_watch_groups(both, &[g1, g2]).unwrap();

        let rescued = db.delete_watch_group(g2).unwrap();
        assert_eq!(rescued, 1, "只有仅在该分组的那只应被救回");
        assert_eq!(
            group_names_of(&db, watch_id_of(&db, "sh600519")),
            vec![DEFAULT_GROUP_NAME.to_string()],
            "孤儿应并入默认分组"
        );
        assert_eq!(
            group_names_of(&db, both),
            vec![DEFAULT_GROUP_NAME.to_string()],
            "本来就在默认分组的股票不应被重复塞进去"
        );
        assert_eq!(db.get_watchlist().unwrap().len(), 2, "删分组不应删掉自选");

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn cannot_delete_the_last_group() {
        let dir = temp_app_dir("lastgroup");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        assert!(db.delete_watch_group(gid).is_err());
        assert_eq!(db.get_watchlist_snapshot().unwrap().groups.len(), 1);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn group_name_is_validated() {
        let dir = temp_app_dir("names");
        let db = Database::open(dir.clone()).unwrap();
        assert!(db.add_watch_group("  ").is_err(), "空名应被拒绝");
        assert!(
            db.add_watch_group(&"长".repeat(GROUP_NAME_MAX_LEN + 1)).is_err(),
            "超长名应被拒绝"
        );
        let g = db.add_watch_group(" 科技 ").unwrap();
        assert_eq!(g.name, "科技", "名称应去除首尾空白");

        assert!(
            db.add_watch_group("银行").is_ok(),
            "同名不同分组是允许的"
        );
        assert!(
            db.rename_watch_group(g.id, "银行").is_err(),
            "重命名撞上已有分组名应被拒绝"
        );
        assert!(db.rename_watch_group(g.id, "银行 ").is_err());
        assert!(
            db.rename_watch_group(g.id, "科技2").is_ok(),
            "重命名成自己以外的名字应成功"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn group_member_reorder_is_scoped_to_the_group() {
        let dir = temp_app_dir("scope");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sh600519", "CN", "贵州茅台", g1).unwrap();
        db.add_watch("sz000001", "CN", "平安银行", g1).unwrap();
        db.add_watch("sz300750", "CN", "宁德时代", g2).unwrap();
        let third = watch_id_of(&db, "sz300750");

        // 置顶第三个分组里的股票，不应扰动 g1 的组内顺序
        let g1_before = db.get_watchlist_snapshot().unwrap().groups[0].watch_ids.clone();
        db.move_group_member_top(g1, third).unwrap();
        let snap = db.get_watchlist_snapshot().unwrap();
        assert_eq!(
            snap.groups[0].watch_ids, g1_before,
            "操作别组的股票不应改动本组顺序"
        );
        assert_eq!(
            snap.groups.iter().find(|g| g.id == g2).unwrap().watch_ids,
            vec![third],
            "该股票本就不在 g1，操作应是 no-op"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn group_member_move_up_down_and_top() {
        let dir = temp_app_dir("sort");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        for code in ["a", "b", "c"] {
            db.add_watch(code, "CN", code, gid).unwrap();
        }
        let (a, b, c) = (
            watch_id_of(&db, "a"),
            watch_id_of(&db, "b"),
            watch_id_of(&db, "c"),
        );
        let order = |db: &Database| db.get_watchlist_snapshot().unwrap().groups[0].watch_ids.clone();
        assert_eq!(order(&db), vec![a, b, c]);

        db.move_group_member_up(gid, a).unwrap();
        assert_eq!(order(&db), vec![a, b, c], "队首上移应是 no-op");

        db.move_group_member_up(gid, b).unwrap();
        assert_eq!(order(&db), vec![b, a, c]);

        db.move_group_member_down(gid, c).unwrap();
        assert_eq!(order(&db), vec![b, a, c], "队尾下移应是 no-op");

        db.move_group_member_top(gid, c).unwrap();
        assert_eq!(order(&db), vec![c, b, a]);

        db.reorder_group_members(gid, &[a, b, c]).unwrap();
        assert_eq!(order(&db), vec![a, b, c]);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn groups_can_be_reordered() {
        let dir = temp_app_dir("gorder");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        let g3 = db.add_watch_group("银行").unwrap().id;

        db.reorder_watch_groups(&[g3, g1, g2]).unwrap();
        let ids: Vec<i64> = db
            .get_watchlist_snapshot()
            .unwrap()
            .groups
            .iter()
            .map(|g| g.id)
            .collect();
        assert_eq!(ids, vec![g3, g1, g2]);
        assert_eq!(
            db.get_watchlist_snapshot().unwrap().default_group_id,
            g3,
            "默认分组 = 展示顺序最靠前的那个，排序后应随之改变"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    /// 不存在的分组 id 必须被挡住：SQLite 默认不启用外键约束，照单写进去就会留下
    /// 一条指向虚空分组的关联 —— 那只股票在池子里、却不在任何分组里，界面上没有
    /// 任何入口能再看到或删掉它。
    #[test]
    fn add_watch_falls_back_to_default_when_group_is_missing() {
        let dir = temp_app_dir("badgroup");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        db.add_watch("sh600519", "CN", "贵州茅台", 9999).unwrap();

        let wid = watch_id_of(&db, "sh600519");
        assert_eq!(
            group_names_of(&db, wid),
            vec![DEFAULT_GROUP_NAME.to_string()],
            "分组不存在时应落到默认分组，而不是留下隐形股票"
        );
        let snap = db.get_watchlist_snapshot().unwrap();
        assert_eq!(snap.groups[0].id, gid);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn set_watch_groups_drops_unknown_group_ids() {
        let dir = temp_app_dir("stale-groups");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g2).unwrap();
        let wid = watch_id_of(&db, "sz300750");
        assert_eq!(group_names_of(&db, wid), vec!["科技".to_string()]);

        // 模拟「菜单打开期间分组被删」：提交里混进一个已不存在的 id
        db.set_watch_groups(wid, &[g1, 9999]).unwrap();
        assert_eq!(
            group_names_of(&db, wid),
            vec![DEFAULT_GROUP_NAME.to_string()],
            "陈旧 id 应被剔除，剩下的有效分组照常生效"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn set_watch_groups_rejects_all_unknown_ids() {
        let dir = temp_app_dir("all-stale");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        db.add_watch("sz300750", "CN", "宁德时代", gid).unwrap();
        let wid = watch_id_of(&db, "sz300750");

        assert!(
            db.set_watch_groups(wid, &[9999, 8888]).is_err(),
            "剔除后一个不剩应按空集合拒绝，而不是把股票变成隐形行"
        );
        assert_eq!(group_names_of(&db, wid), vec![DEFAULT_GROUP_NAME.to_string()]);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn remove_watch_from_foreign_group_is_noop() {
        let dir = temp_app_dir("foreign");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        let wid = watch_id_of(&db, "sz300750");

        // 它不在 g2 里，从 g2 移除应当是空操作，且**不能**把它当孤儿删掉
        let deleted = db.remove_watch_from_group(g2, wid).unwrap();
        assert!(!deleted, "不在该分组时不应触发孤儿删除");
        assert_eq!(db.get_watchlist().unwrap().len(), 1);
        assert_eq!(group_names_of(&db, wid), vec![DEFAULT_GROUP_NAME.to_string()]);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn delete_group_without_orphans_rescues_nothing() {
        let dir = temp_app_dir("no-orphan");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        let wid = watch_id_of(&db, "sz300750");
        db.set_watch_groups(wid, &[g1, g2]).unwrap();

        let rescued = db.delete_watch_group(g2).unwrap();
        assert_eq!(rescued, 0, "股票还在默认分组里，不该被当成孤儿再塞一次");
        assert_eq!(group_names_of(&db, wid), vec![DEFAULT_GROUP_NAME.to_string()]);

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn rename_group_to_its_own_name_is_allowed() {
        let dir = temp_app_dir("self-rename");
        let db = Database::open(dir.clone()).unwrap();
        let g = db.add_watch_group("科技").unwrap();
        assert!(
            db.rename_watch_group(g.id, "科技").is_ok(),
            "重命名成自己当前的名字应允许（否则前端失焦提交会莫名报错）"
        );
        assert_eq!(db.get_watchlist_snapshot().unwrap().groups[1].name, "科技");

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    // ── 行情条播报范围的批量与重排 ──

    #[test]
    fn bulk_disable_clears_the_flag_without_reordering() {
        let dir = temp_app_dir("bulk-off");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        for code in ["sh600519", "sz000001", "sz300750"] {
            db.add_watch(code, "CN", code, gid).unwrap();
        }
        let items_before = db.get_watchlist().unwrap();
        let ids: Vec<i64> = items_before.iter().map(|i| i.id).collect();

        db.set_ticker_enabled_bulk(&ids, false).unwrap();
        let after = db.get_watchlist().unwrap();
        assert!(after.iter().all(|i| !i.ticker_enabled), "全部应关闭播报");
        // 关闭不写 ticker_order —— 重新开启时会按新顺序追加到队尾，
        // 关闭动作去动它只会让"关一次再开一次"就换一遍顺序
        for before in &items_before {
            let now = after.iter().find(|i| i.id == before.id).unwrap();
            assert_eq!(now.ticker_order, before.ticker_order);
        }

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn reorder_ticker_writes_sequential_positions() {
        let dir = temp_app_dir("reorder-ticker");
        let db = Database::open(dir.clone()).unwrap();
        let gid = db.get_watchlist_snapshot().unwrap().default_group_id;
        for code in ["a", "b", "c"] {
            db.add_watch(code, "CN", code, gid).unwrap();
        }
        let (a, b, c) = (
            watch_id_of(&db, "a"),
            watch_id_of(&db, "b"),
            watch_id_of(&db, "c"),
        );

        db.reorder_ticker(&[c, a, b]).unwrap();
        let items = db.get_watchlist().unwrap();
        let order = |id: i64| items.iter().find(|i| i.id == id).unwrap().ticker_order;
        assert_eq!((order(c), order(a), order(b)), (0, 1, 2));

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn remove_watch_clears_all_memberships() {        let dir = temp_app_dir("purge");
        let db = Database::open(dir.clone()).unwrap();
        let g1 = db.get_watchlist_snapshot().unwrap().default_group_id;
        let g2 = db.add_watch_group("科技").unwrap().id;
        db.add_watch("sz300750", "CN", "宁德时代", g1).unwrap();
        let wid = watch_id_of(&db, "sz300750");
        db.set_watch_groups(wid, &[g1, g2]).unwrap();

        db.remove_watch("sz300750", "CN").unwrap();
        let snap = db.get_watchlist_snapshot().unwrap();
        assert!(snap.items.is_empty());
        assert!(
            snap.groups.iter().all(|g| g.watch_ids.is_empty()),
            "彻底删除应清掉全部关联，不留悬空 watch_id"
        );

        drop(db);
        std::fs::remove_dir_all(&dir).expect("临时测试目录应可清理");
    }

    #[test]
    fn default_index_codes_match_datasource() {
        // 两处字面量必须同步:db 的默认设置是 JSON 字符串，datasource 的是 &[&str]。
        // 用测试而不是 const 引用，是因为 JSON 字面量没法在 const 上下文里拼出来。
        let parsed: Vec<String> = serde_json::from_str(DEFAULT_INDEX_CODES_JSON)
            .expect("默认指数列表应是合法 JSON 数组");
        let expected: Vec<String> = crate::datasource::DEFAULT_INDEX_CODES
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(parsed, expected);

        for code in &parsed {
            assert!(
                crate::datasource::INDEX_CODES.split(',').any(|c| c == code),
                "默认勾选的 {} 必须存在于指数候选池中",
                code
            );
        }
    }

    /// `DEFAULT_SETTINGS` 的键名是**前后端之间的线格式**：前端
    /// `stores/settings.ts` / `utils/prefs.ts` 按这些字符串取值。
    ///
    /// 改动常量的**值**（例如把 `sector_top_n` 改成 `sector_topN`）不会影响 Rust
    /// 编译，前端也不会报错 —— `parseCount` 读不到就退回默认值，症状是「设置页改得
    /// 动，看盘界面不生效」。这里把线格式钉死。
    #[test]
    fn default_settings_keys_are_pinned() {
        let keys: Vec<&str> = Database::DEFAULT_SETTINGS.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            vec![
                "active_datasource",
                "theme",
                "ticker_visible",
                "auto_launch",
                "index_codes",
                "market_overview_visible",
                "sector_top_n",
                "ticker_transparent",
                "ticker_items_per_page",
                "watchlist_columns",
                "watchlist_default_sort",
                "color_scheme",
            ]
        );
    }

    #[test]
    fn default_settings_have_no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for (k, _) in Database::DEFAULT_SETTINGS {
            assert!(seen.insert(*k), "默认设置里 {} 出现了两次，后者会覆盖前者", k);
        }
    }

    /// 迁移标记必须**不存在**才会触发迁移。把它写进默认值会让
    /// `migrate_watch_groups` 的一次性灌入永远不执行 —— 老用户的全部自选
    /// 会落在所有分组之外，界面上看不见。这条测试挡住"顺手把所有键都补进默认值"
    /// 这类看起来无害的重构。
    #[test]
    fn migration_marker_is_not_a_default() {
        assert!(
            !Database::DEFAULT_SETTINGS
                .iter()
                .any(|(k, _)| *k == keys::GROUPS_MIGRATED),
            "把 {} 写进默认值会让分组迁移永不执行",
            keys::GROUPS_MIGRATED
        );
    }

    /// 布尔型默认值必须是 "0"/"1"。
    ///
    /// 前端 `parseBool`（src/utils/prefs.ts）只认这两个值，其余一律退回 fallback。
    /// 写成 "true" 不会报错，只会让设置页显示的值与实际生效的值不一致。
    #[test]
    fn boolean_defaults_use_the_0_1_wire_format() {
        for key in [
            keys::MARKET_OVERVIEW_VISIBLE,
            keys::TICKER_VISIBLE,
            keys::TICKER_TRANSPARENT,
        ] {
            let v = Database::DEFAULT_SETTINGS
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| *v)
                .unwrap_or_else(|| panic!("{} 不在默认值表里", key));
            assert!(v == "0" || v == "1", "{} 的默认值 {:?} 应为 \"0\" 或 \"1\"", key, v);
        }
    }

    /// 数值型默认值必须能解析成正整数 —— 前端用 `parseInt` 读，写出 "abc"
    /// 只会让设置页静默显示兜底值。
    #[test]
    fn numeric_defaults_parse() {
        for key in [keys::SECTOR_TOP_N, keys::TICKER_ITEMS_PER_PAGE] {
            let v = Database::DEFAULT_SETTINGS
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| *v)
                .unwrap_or_else(|| panic!("{} 不在默认值表里", key));
            assert!(v.parse::<u32>().is_ok(), "{} 的默认值 {:?} 应是正整数", key, v);
        }
    }

    #[test]
    fn default_watchlist_columns_exclude_ticker_toggle() {        let parsed: Vec<String> = serde_json::from_str(DEFAULT_WATCHLIST_COLUMNS_JSON)
            .expect("默认列应是合法 JSON 数组");
        assert!(
            !parsed.iter().any(|c| c == "ticker_enabled"),
            "行情条播报开关已收归设置页，不应再是表格列"
        );
        assert!(parsed.contains(&"code".to_string()));
        assert!(parsed.contains(&"name".to_string()));
    }
}
