//! Linux 启动后端选择：在 Wayland 会话里改用 XWayland（X11）启动自己。
//!
//! ## 为什么必须这样
//!
//! 行情条要「始终置顶」，而它靠的是 GTK 的 `set_keep_above` —— 在 X11 上这是
//! `_NET_WM_STATE_ABOVE`，WM 会照办；在 Wayland 上**没有任何对应协议**：
//! 置顶是合成器的权力，客户端无权索取，GTK 调用它只是静默无效。也就是说
//! 「置顶」这个需求在 Wayland 原生会话里无解，只能走 XWayland。
//!
//! ## 为什么不能只靠启动图标
//!
//! 用户此前用桌面图标的 `Exec=env GDK_BACKEND=x11 quant-desktop` 达到置顶，
//! 但**其它启动路径没有这个前缀**：设置页里的「开机自启」是
//! tauri-plugin-autostart 生成的 `~/.config/autostart/*.desktop`（裸 `Exec=`），
//! 终端里敲命令启动也一样。结果就是「图标启动能置顶、自启后不能」。
//!
//! 所以判定放进应用自己：检测到 Wayland 会话就带着 `GDK_BACKEND=x11` 重新
//! `exec` 自己（`exec` 替换进程映像，不留父进程、不双开）。这样一来图标、
//! 自启、终端三条路径行为一致，图标里那个前缀也就不再必要。
//!
//! 附带好处：X11 下 `outer_position()` 读得到真实坐标，行情条的位置记忆
//! （`ticker_x`/`ticker_y`）才真正生效；Wayland 原生会话里它恒为 (0,0)。
//!
//! ## 检测依据与环境变量
//!
//! - 是否 Wayland：`XDG_SESSION_TYPE=wayland` 或 `WAYLAND_DISPLAY` 非空。
//! - 能否切：`DISPLAY` 指向本地且 `/tmp/.X11-unix/X<n>` 存在（有 XWayland）。
//!   没有 XWayland 时留在 Wayland：置顶无解，但至少应用能启动。
//! - `QUANT_BACKEND=x11|wayland`：手动指定，用于排查（默认 auto）。
//! - `QUANT_BACKEND_REEXEC=1`：本模块给子进程打的标记，防无限重入。
//!
//! 注意：这里**不**按「Wayland 版本」做分支。置顶缺的不是某个版本才有的
//! 协议，而是从来就不存在 —— 任何版本的 Wayland 原生会话都做不到，所以
//! 判定只看会话类型 + XWayland 是否可用。

use std::path::{Path, PathBuf};

/// 判定所需的全部环境输入（纯数据，便于单测）。
#[derive(Debug, Default)]
pub(crate) struct BackendEnv {
    pub session_type: Option<String>,
    pub wayland_display: Option<String>,
    pub display: Option<String>,
    pub gdk_backend: Option<String>,
    /// `QUANT_BACKEND`：`auto`（默认）/ `x11` / `wayland`
    pub forced: Option<String>,
    /// `QUANT_BACKEND_REEXEC`：已经由本模块切换过一次
    pub reexec: bool,
}

impl BackendEnv {
    fn from_process() -> Self {
        let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Self {
            session_type: get("XDG_SESSION_TYPE"),
            wayland_display: get("WAYLAND_DISPLAY"),
            display: get("DISPLAY"),
            gdk_backend: get("GDK_BACKEND"),
            forced: get("QUANT_BACKEND"),
            reexec: get("QUANT_BACKEND_REEXEC").is_some(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// 保持当前后端（Wayland 原生，或本来就已是 X11）
    Keep(&'static str),
    /// 带着 GDK_BACKEND=x11 重新 exec
    SwitchToX11(&'static str),
}

pub(crate) fn is_wayland_session(env: &BackendEnv) -> bool {
    env.session_type.as_deref() == Some("wayland")
        || env.wayland_display.as_ref().is_some_and(|v| !v.trim().is_empty())
}

/// `DISPLAY` 是否指向本机 X server，且该 display 的 socket 存在。
fn local_x_socket_present(display: &str, exists: impl Fn(&Path) -> bool) -> bool {
    // ":0" / ":0.0" / "unix:0" 是本地；"host:0" 是远程（XWayland 不会那样）。
    let Some(rest) = display.strip_prefix(':').or_else(|| display.strip_prefix("unix:")) else {
        return false;
    };
    let number = rest.split('.').next().unwrap_or("");
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    exists(Path::new(&format!("/tmp/.X11-unix/X{number}")))
}

/// 纯判定：要不要切 X11，以及为什么（原因会写进日志，便于用户自查）。
pub(crate) fn decide(env: &BackendEnv, x_socket_present: bool) -> Decision {
    // 已经切过一次就停在 X11，别无限重入。
    if env.reexec {
        return Decision::Keep("已按 Wayland 会话切换过 XWayland");
    }
    // 显式指定后端优先于一切自动判定。
    match env.forced.as_deref() {
        Some("wayland") => return Decision::Keep("QUANT_BACKEND=wayland"),
        Some("x11") => {
            return if x_socket_present {
                Decision::SwitchToX11("QUANT_BACKEND=x11")
            } else {
                Decision::Keep("QUANT_BACKEND=x11 但找不到 X server，退回 Wayland")
            };
        }
        _ => {}
    }
    // 启动器已经指定了后端（例如旧图标的 `Exec=env GDK_BACKEND=x11 …`）就不插手。
    if let Some(backend) = env.gdk_backend.as_deref() {
        return if backend.eq_ignore_ascii_case("x11") {
            Decision::Keep("GDK_BACKEND 已由启动环境指定为 x11")
        } else {
            Decision::Keep("GDK_BACKEND 已由启动环境指定，尊重该设置")
        };
    }
    if !is_wayland_session(env) {
        return Decision::Keep("X11 会话");
    }
    if !x_socket_present {
        return Decision::Keep("Wayland 会话但没有 XWayland，置顶不可用");
    }
    Decision::SwitchToX11("Wayland 会话下 keep_above 无协议支持，改用 XWayland")
}

/// GNOME 的 XWayland 授权文件（会话环境里一般已有 `XAUTHORITY`，
/// 但自启/部分启动器未必带上；没有它 GTK 连不上 X，应用会起不来）。
fn find_xwayland_auth(runtime_dir: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = runtime_dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(".mutter-Xwaylandauth"))
                {
                    candidates.push(path);
                }
            }
        }
    }
    if let Some(home) = home {
        candidates.push(Path::new(home).join(".Xauthority"));
    }
    candidates.into_iter().find(|p| p.exists())
}

fn display_x_socket_present(display: Option<&str>) -> bool {
    match display {
        Some(d) => local_x_socket_present(d, |p| p.exists()),
        None => false,
    }
}

/// 启动最早期调用：必要且可行时带着 `GDK_BACKEND=x11` 重新 exec 自己。
///
/// 必须在 GTK/Tauri 初始化**之前**（`run()` 的第一句），否则后端已经定了。
/// exec 成功不返回；失败就退回 Wayland 继续启动（宁可没有置顶，也不能起不来）。
pub(crate) fn ensure_x11_backend() {
    let env = BackendEnv::from_process();
    let x_available = display_x_socket_present(env.display.as_deref());
    let Decision::SwitchToX11(reason) = decide(&env, x_available) else {
        return;
    };

    let exe = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("quant-desktop"));
    let mut command = std::process::Command::new(&exe);
    command
        .args(std::env::args_os().skip(1))
        .env("GDK_BACKEND", "x11")
        .env("QUANT_BACKEND_REEXEC", "1");

    if std::env::var_os("XAUTHORITY").is_none() {
        let runtime = std::env::var("XDG_RUNTIME_DIR").ok();
        let home = std::env::var("HOME").ok();
        if let Some(auth) = find_xwayland_auth(runtime.as_deref(), home.as_deref()) {
            command.env("XAUTHORITY", auth);
        }
    }

    use std::os::unix::process::CommandExt;
    let err = command.exec();
    // exec 只有失败才返回。
    eprintln!(
        "QuantDesktop: 切到 XWayland 失败（{err}），继续用 Wayland 启动 —— 行情条将无法置顶（原因：{reason}）"
    );
}

/// 一行后端说明，交给日志（logger 在 setup 里才就绪，所以判定结果延后记录）。
pub(crate) fn backend_summary() -> String {
    let env = BackendEnv::from_process();
    let x_available = display_x_socket_present(env.display.as_deref());
    let decision = decide(&env, x_available);
    let forced = env.forced.clone().unwrap_or_else(|| "auto".to_string());
    let session = env.session_type.clone().unwrap_or_else(|| "unknown".to_string());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "-".to_string());
    match decision {
        Decision::SwitchToX11(reason) => format!(
            "启动后端: X11（待切换）| 会话 {session} / {desktop} | QUANT_BACKEND={forced} | {reason}"
        ),
        Decision::Keep(reason) => {
            let effective = env.gdk_backend.clone().unwrap_or_else(|| {
                if is_wayland_session(&env) { "wayland".into() } else { "x11".into() }
            });
            format!(
                "启动后端: {effective} | 会话 {session} / {desktop} | QUANT_BACKEND={forced} | {reason}"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wayland_env() -> BackendEnv {
        BackendEnv {
            session_type: Some("wayland".into()),
            wayland_display: Some("wayland-0".into()),
            display: Some(":0".into()),
            ..Default::default()
        }
    }

    #[test]
    fn wayland_session_with_xwayland_switches_to_x11() {
        let decision = decide(&wayland_env(), true);
        assert!(matches!(decision, Decision::SwitchToX11(_)), "{decision:?}");
    }

    /// 没有 XWayland 时不能强切：GDK_BACKEND=x11 连不上 X 会直接起不来，
    /// 宁可留在 Wayland（置顶失效但应用可用）。
    #[test]
    fn wayland_without_xwayland_stays() {
        assert!(matches!(decide(&wayland_env(), false), Decision::Keep(_)));
    }

    #[test]
    fn x11_session_keeps_x11() {
        let env = BackendEnv {
            session_type: Some("x11".into()),
            display: Some(":0".into()),
            ..Default::default()
        };
        assert_eq!(decide(&env, true), Decision::Keep("X11 会话"));
    }

    /// 启动器自己指定了 GDK_BACKEND（老图标 `Exec=env GDK_BACKEND=x11 …`）就尊重它，
    /// 不要再套一层 exec。
    #[test]
    fn explicit_backend_is_respected() {
        let mut env = wayland_env();
        env.gdk_backend = Some("x11".into());
        assert!(matches!(decide(&env, true), Decision::Keep(_)));

        let mut env = wayland_env();
        env.gdk_backend = Some("wayland".into());
        assert!(matches!(decide(&env, true), Decision::Keep(_)));
    }

    /// QUANT_BACKEND 是排查开关，优先于自动判定。
    #[test]
    fn forced_backend_wins() {
        let mut env = wayland_env();
        env.forced = Some("wayland".into());
        assert_eq!(decide(&env, true), Decision::Keep("QUANT_BACKEND=wayland"));

        let mut env = wayland_env();
        env.forced = Some("x11".into());
        assert!(matches!(decide(&env, true), Decision::SwitchToX11(_)));

        // 强制 x11 但没有 X server：退回 Wayland 而不是起不来
        assert!(matches!(decide(&env, false), Decision::Keep(_)));
    }

    /// 防重入：exec 出来的子进程带着标记，不能再切一次。
    #[test]
    fn reexec_marker_stops_looping() {
        let mut env = wayland_env();
        env.reexec = true;
        assert!(matches!(decide(&env, true), Decision::Keep(_)));
    }

    /// 只在本地 display 上判断 socket；远程 X（如 SSH 转发的 `somehost:10.0`）不碰。
    #[test]
    fn local_display_parsing() {
        let present = |_: &Path| true;
        assert!(local_x_socket_present(":0", present));
        assert!(local_x_socket_present(":0.0", present));
        assert!(local_x_socket_present(":12", present));
        assert!(local_x_socket_present("unix:0", present));
        assert!(!local_x_socket_present("somehost:10.0", present));
        assert!(!local_x_socket_present(":abc", present));
        assert!(!local_x_socket_present("", present));

        let absent = |_: &Path| false;
        assert!(!local_x_socket_present(":0", absent));
    }

    #[test]
    fn wayland_detection() {
        assert!(is_wayland_session(&wayland_env()));

        let only_display = BackendEnv {
            session_type: None,
            wayland_display: Some("wayland-1".into()),
            ..Default::default()
        };
        assert!(is_wayland_session(&only_display));

        let x11 = BackendEnv {
            session_type: Some("x11".into()),
            ..Default::default()
        };
        assert!(!is_wayland_session(&x11));
    }
}
