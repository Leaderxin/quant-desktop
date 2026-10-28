//! 日志配置与日志相关的小工具。
//!
//! 三件事必须一起做对，否则用户报障时拿不到证据：
//! - **时间用本机时区**：用户说的是本机钟点，日志写成 UTC 就差 8 小时；
//! - **文件追加而不是每次启动清空**：这样上一次崩溃的现场还在；
//! - **超过上限先轮转**：否则长期运行的进程会把日志写到几百 MB。
//!
//! 级别默认 `info`，可用环境变量 `QUANT_LOG`（error/warn/info/debug/trace）
//! 临时放开，不需要重新打包。

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use log::LevelFilter;
use simplelog::{Config, ConfigBuilder};

/// 控制日志级别的环境变量名。
pub const LEVEL_ENV: &str = "QUANT_LOG";
/// 未设置 `QUANT_LOG` 时的级别。
pub const DEFAULT_LEVEL: LevelFilter = LevelFilter::Info;
/// 单个日志文件的上限；超过后改名为 `.1`，重新开一个。
pub const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
/// 同一条高频告警的抑制窗口。
pub const THROTTLE_WINDOW: Duration = Duration::from_secs(300);
/// 单条前端日志最多保留的字符数（前端堆栈可能很长）。
pub const MAX_WEB_LOG_CHARS: usize = 4000;

/// 解析 `QUANT_LOG` 的取值。大小写与首尾空白都容忍，无法识别时回退 info。
pub fn parse_level(raw: Option<&str>) -> LevelFilter {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        Some("info") => LevelFilter::Info,
        // 空串 / 拼错的值都按默认级别走：日志配置坏了不该让应用起不来。
        _ => DEFAULT_LEVEL,
    }
}

/// 读取环境变量里的级别。
pub fn requested_level() -> LevelFilter {
    let raw = std::env::var(LEVEL_ENV).ok();
    parse_level(raw.as_deref())
}

/// 两个 logger 共用的格式配置 —— 重点是时间用本机时区。
pub fn config() -> Config {
    let mut builder = ConfigBuilder::new();
    // 多线程环境下取本地偏移可能失败（time crate 的已知限制）；失败就退回
    // UTC，不能因为拿不到时区就不记日志。
    let _ = builder.set_time_offset_to_local();
    builder.build()
}

/// 打开（必要时先轮转）日志文件。追加写：上一次运行的内容要留着。
pub fn open_log_file(app_dir: &Path) -> std::io::Result<RotatingFile> {
    fs::create_dir_all(app_dir)?;
    RotatingFile::open(app_dir.join("quant-desktop.log"), MAX_LOG_BYTES)
}

/// 到上限就轮转的写入口，交给 simplelog 的 `WriteLogger`。
///
/// 只在启动时查一次大小是不够的：托盘常驻进程可以连续跑几天，中间没人会
/// 重新打开这个文件。轮转要改名，而 Windows 上还开着的文件不能改名 ——
/// 所以先放掉句柄，改名，再以追加模式重新打开。
pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    file: Option<File>,
    /// 当前文件已写字节数；每次启动从文件长度续上。
    written: u64,
    /// 上一次写是否以换行结束。simplelog 一条记录会分几次 `write`，
    /// 只在记录边界轮转，否则会把时间戳留在旧文件、正文留在新文件。
    at_line_start: bool,
}

impl RotatingFile {
    pub fn open(path: PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        // 上一次运行留下的超大文件在这里先挪走，而不是等下一次写才轮转。
        if let Err(e) = rotate_if_large(&path, max_bytes) {
            eprintln!("QuantDesktop: log rotation skipped: {e}");
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata()?.len();
        Ok(Self {
            path,
            max_bytes,
            file: Some(file),
            written,
            at_line_start: true,
        })
    }

    /// 关掉句柄 → 旧文件改名 → 重开新文件。失败时尽量保住继续写的能力。
    fn rotate(&mut self) -> std::io::Result<()> {
        self.file = None;
        let rotated = rotated_path(&self.path);
        let rename = (|| {
            if rotated.exists() {
                fs::remove_file(&rotated)?;
            }
            fs::rename(&self.path, &rotated)
        })();

        let file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        if rename.is_err() {
            // 改名失败通常意味着文件被别的进程占着（编辑器 / 杀软扫描）。
            // 退化为清空：宁可丢历史日志，也不能让「有上限」变成没有上限。
            let _ = file.set_len(0);
        }
        self.file = Some(file);
        self.written = 0;
        rename
    }
}

impl std::io::Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // 上一次重开失败（磁盘满、权限等）时每次写再试一次：条件恢复后日志能自己接上。
        if self.file.is_none() {
            if let Ok(file) = OpenOptions::new().create(true).append(true).open(&self.path) {
                self.written = file.metadata().map(|m| m.len()).unwrap_or(0);
                self.file = Some(file);
            }
        }
        if self.at_line_start && self.written + buf.len() as u64 > self.max_bytes {
            // 轮转失败也继续写：日志有点乱，好过整段丢失。
            let _ = self.rotate();
        }
        let file = self.file.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "log file unavailable")
        })?;
        let n = file.write(buf)?;
        self.written += n as u64;
        self.at_line_start = buf[..n].ends_with(b"\n");
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// `<name>.log` → `<name>.log.1`
fn rotated_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

/// 日志文件达到 `max_bytes` 时改名为 `<原名>.1`（覆盖旧的 `.1`）。
///
/// 返回 `Some(rotated_path)` 表示发生了轮转。文件不存在不算错误 —— 首次启动
/// 就是这种情况。
pub fn rotate_if_large(path: &Path, max_bytes: u64) -> std::io::Result<Option<PathBuf>> {
    let len = match fs::metadata(path) {
        Ok(meta) => meta.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if len < max_bytes {
        return Ok(None);
    }

    let rotated = rotated_path(path);
    // Windows 上 rename 到已存在的目标会失败，先删掉上一轮的备份。
    if rotated.exists() {
        fs::remove_file(&rotated)?;
    }
    fs::rename(path, &rotated)?;
    Ok(Some(rotated))
}

/// 让 panic 也进日志文件。
///
/// release 包是 GUI 进程，stderr 不可见；`expect`/`unwrap` 直接 panic 时，
/// 日志里只剩前一行「starting」，看起来像应用根本没启动。这层 hook 把
/// panic 位置与消息写进文件，同时保留默认 hook 的 stderr 输出。
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("Panic: {}", info);
        default_hook(info);
    }));
}

/// 前端日志落盘。级别不在词表里时按 info 处理。
pub fn log_from_web(level: &str, message: &str) {
    let msg = truncate_chars(message, MAX_WEB_LOG_CHARS);
    match level {
        "error" => log::error!("[web] {}", msg),
        "warn" => log::warn!("[web] {}", msg),
        "debug" => log::debug!("[web] {}", msg),
        _ => log::info!("[web] {}", msg),
    }
}

/// 按字符（不是字节）截断，避免把多字节汉字切坏。
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

static THROTTLE_SEEN: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

/// 同一 `key` 在 [`THROTTLE_WINDOW`] 内只放行一次。
///
/// `now` 由调用方传入，是为了让测试不依赖真实时钟。
pub fn allow_log(key: &str, now: Instant) -> bool {
    let seen = THROTTLE_SEEN.get_or_init(|| Mutex::new(HashMap::new()));
    let mut seen = seen.lock().unwrap_or_else(|e| e.into_inner());
    // 键里带股票代码，长跑下来可能攒到几百条；偶尔把过期的清掉，别让它无限增长。
    if seen.len() >= 512 {
        seen.retain(|_, t| now.duration_since(*t) < THROTTLE_WINDOW);
    }
    match seen.get(key) {
        Some(prev) if now.duration_since(*prev) < THROTTLE_WINDOW => false,
        _ => {
            seen.insert(key.to_string(), now);
            true
        }
    }
}

/// 记一条 warn，但同一个 key 五分钟内只记一次。
///
/// 轮询型接口（市场概览、深度）断网时会每轮都失败；不抑制的话日志会被同一条
/// 消息刷满，真正有用的那几条反而找不到。被抑制的重复仍留在 debug 级别 ——
/// 用 `QUANT_LOG=debug` 排查时能看到每一次，而不是只看到第一条。
pub fn warn_throttled(key: &str, message: impl FnOnce() -> String) {
    let message = message();
    if allow_log(key, Instant::now()) {
        log::warn!("{}", message);
    } else {
        log::debug!("{} (repeated)", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "quant-desktop-log-{}-{}-{}",
            tag,
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn parses_known_levels() {
        assert_eq!(parse_level(Some("error")), LevelFilter::Error);
        assert_eq!(parse_level(Some("warn")), LevelFilter::Warn);
        assert_eq!(parse_level(Some("info")), LevelFilter::Info);
        assert_eq!(parse_level(Some("debug")), LevelFilter::Debug);
        assert_eq!(parse_level(Some("trace")), LevelFilter::Trace);
    }

    #[test]
    fn level_parsing_is_case_insensitive_and_trims() {
        assert_eq!(parse_level(Some("  DEBUG ")), LevelFilter::Debug);
        assert_eq!(parse_level(Some("Warn")), LevelFilter::Warn);
    }

    #[test]
    fn missing_or_unknown_level_falls_back_to_info() {
        assert_eq!(parse_level(None), DEFAULT_LEVEL);
        assert_eq!(parse_level(Some("")), DEFAULT_LEVEL);
        assert_eq!(parse_level(Some("verbose")), DEFAULT_LEVEL);
    }

    #[test]
    fn truncate_keeps_short_text_intact() {
        assert_eq!(truncate_chars("short", 10), "short");
        assert_eq!(truncate_chars("", 10), "");
    }

    #[test]
    fn truncate_counts_chars_not_bytes() {
        // 12 个汉字 = 36 字节；按字节切会切出半个字符
        let s = "日志记录应该按字符截断";
        let cut = truncate_chars(s, 4);
        assert_eq!(cut, "日志记录");
        assert!(cut.is_char_boundary(cut.len()));
    }

    #[test]
    fn rotation_ignores_missing_and_small_files() {
        let dir = temp_dir("rotate-small");

        let missing = dir.join("missing.log");
        assert_eq!(rotate_if_large(&missing, 1024).unwrap(), None);

        let small = dir.join("small.log");
        std::fs::write(&small, b"hello").unwrap();
        assert_eq!(rotate_if_large(&small, 1024).unwrap(), None);
        assert!(small.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotation_renames_file_at_limit() {
        let dir = temp_dir("rotate-large");
        let path = dir.join("quant-desktop.log");
        let mut f = File::create(&path).unwrap();
        f.write_all(&vec![b'x'; 32]).unwrap();
        drop(f);

        let rotated = rotate_if_large(&path, 32).unwrap().expect("should rotate");
        assert_eq!(rotated, dir.join("quant-desktop.log.1"));
        assert!(!path.exists());
        assert_eq!(std::fs::metadata(&rotated).unwrap().len(), 32);

        // 再轮转一次会覆盖旧的 .1，而不是报错
        std::fs::write(&path, vec![b'y'; 40]).unwrap();
        let rotated_again = rotate_if_large(&path, 32).unwrap().expect("should rotate again");
        assert_eq!(rotated_again, rotated);
        assert_eq!(std::fs::metadata(&rotated).unwrap().len(), 40);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_log_file_appends_instead_of_truncating() {
        let dir = temp_dir("append");
        drop(open_log_file(&dir).unwrap());
        std::fs::write(dir.join("quant-desktop.log"), b"previous run\n").unwrap();

        let mut f = open_log_file(&dir).unwrap();
        f.write_all(b"this run\n").unwrap();
        drop(f);

        let content = std::fs::read_to_string(dir.join("quant-desktop.log")).unwrap();
        assert!(content.contains("previous run"), "append mode must keep old content");
        assert!(content.contains("this run"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotating_file_rotates_at_record_boundaries_only() {
        let dir = temp_dir("rotate-runtime");
        let path = dir.join("quant-desktop.log");
        let mut writer = RotatingFile::open(path.clone(), 16).unwrap();

        // 一条记录分两次写：8 + 5 字节，超过 16，但第二次不是记录开头 —— 不轮转
        writer.write_all(b"12345678").unwrap();
        writer.flush().unwrap();
        writer.write_all(b"abcd\n").unwrap();
        writer.flush().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "12345678abcd\n");
        assert!(!dir.join("quant-desktop.log.1").exists());

        // 下一条记录从头开始，且已经超过上限：先轮转
        writer.write_all(b"next\n").unwrap();
        writer.flush().unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("quant-desktop.log.1")).unwrap(),
            "12345678abcd\n"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "next\n");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn throttle_allows_once_per_window() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);

        let start = Instant::now();
        let key = format!(
            "test-throttle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );

        assert!(allow_log(&key, start));
        assert!(!allow_log(&key, start + Duration::from_secs(1)));
        assert!(!allow_log(&key, start + THROTTLE_WINDOW - Duration::from_secs(1)));
        assert!(allow_log(&key, start + THROTTLE_WINDOW));

        // 不同 key 互不影响
        assert!(allow_log(&format!("{key}-other"), start));
    }
}
