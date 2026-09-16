use tauri::{Manager, Emitter};
use std::sync::Arc;
use crate::db::Database;

pub use super::ticker_geometry::ResizeEdge;
use super::ticker_geometry::{Bounds, clamp_bounds, resize_bounds_with_header};

pub fn market_visible(db: &Database) -> bool {
    db.get_setting("ticker_market_visible").ok().flatten().as_deref() == Some("1")
}

#[tauri::command]
pub fn get_ticker_market_visible(db: tauri::State<'_, Arc<Database>>) -> bool { market_visible(&db) }

#[tauri::command]
pub fn set_ticker_market_visible(app: tauri::AppHandle, db: tauri::State<'_, Arc<Database>>, visible: bool) -> Result<(), String> {
    if market_visible(&db) == visible { return Ok(()); }
    let window = app.get_webview_window("ticker").ok_or("Ticker window not found")?;
    let current = outer_bounds(&window)?;
    let inner = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let area = work_area(&window, false)?;
    let rows = (((inner.height as f64 / scale - 8.0) / 15.0).round() as u32).saturating_sub(u32::from(market_visible(&db)));
    let height = ((ticker_height(rows) + if visible { 15 } else { 0 }) as f64 * scale).round() as u32 + current.height.saturating_sub(inner.height);
    if height > area.height { return Err("空间不足，无法增加大盘行情行".into()); }
    let saved = db.get_setting("ticker_market_anchor").ok().flatten()
        .and_then(|s| serde_json::from_str::<(i32, u32, bool)>(&s).ok());
    let bottom = saved.filter(|(y,h,_)| *y == current.y && *h == current.height)
        .map(|(_,_,bottom)| bottom)
        .unwrap_or(current.y as i64 * 2 + current.height as i64 > area.y as i64 * 2 + area.height as i64);
    let bounds = clamp_bounds(Bounds { height, y: if bottom { current.y + current.height as i32 - height as i32 } else { current.y }, ..current }, area);
    let frame = (current.width.saturating_sub(inner.width), current.height.saturating_sub(inner.height));
    apply_bounds(&window, bounds, frame)?;
    if let Err(e) = db.set_setting("ticker_market_visible", if visible { "1" } else { "0" }) {
        let _ = apply_bounds(&window, current, frame);
        return Err(e.to_string());
    }
    let _ = db.set_setting("ticker_market_anchor", &serde_json::to_string(&(bounds.y,bounds.height,bottom)).unwrap());
    let _ = app.emit("ticker-market-changed", visible);
    Ok(())
}

fn market_height(window: &tauri::WebviewWindow) -> u32 {
    if market_visible(&window.state::<Arc<Database>>()) { 15 } else { 0 }
}

pub fn ticker_height(rows: u32) -> u32 {
    8 + rows.clamp(2, 30) * 15
}

fn outer_bounds(window: &tauri::WebviewWindow) -> Result<Bounds, String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    Ok(Bounds { x: pos.x, y: pos.y, width: size.width, height: size.height })
}

fn work_area(window: &tauri::WebviewWindow, primary: bool) -> Result<Bounds, String> {
    let monitor = if primary { window.primary_monitor() } else { window.current_monitor() }
        .map_err(|e| e.to_string())?
        .or(window.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("未找到可用显示器")?;
    let area = monitor.work_area();
    // Also leave a small gap for auto-hidden taskbars and resize handles.
    let margin = (4.0 * monitor.scale_factor()).ceil() as u32;
    Ok(Bounds {
        x: area.position.x + margin as i32, y: area.position.y + margin as i32,
        width: area.size.width.saturating_sub(2 * margin),
        height: area.size.height.saturating_sub(2 * margin),
    })
}

fn apply_bounds(window: &tauri::WebviewWindow, bounds: Bounds, frame: (u32, u32)) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let _ = frame;
        unsafe { crate::windows_util::set_bounds(hwnd.0 as isize, bounds.x, bounds.y, bounds.width, bounds.height) }
    }
    #[cfg(not(target_os = "windows"))]
    {
        window.set_size(tauri::PhysicalSize::new(bounds.width.saturating_sub(frame.0), bounds.height.saturating_sub(frame.1)))
            .map_err(|e| e.to_string())?;
        window.set_position(tauri::PhysicalPosition::new(bounds.x, bounds.y)).map_err(|e| e.to_string())
    }
}

pub fn resize_ticker(window: &tauri::WebviewWindow, rows: u32, edge: ResizeEdge) -> Result<u32, String> {
    resize_ticker_at_least(window, rows, edge, 2)
}

fn resize_ticker_at_least(window: &tauri::WebviewWindow, rows: u32, edge: ResizeEdge, minimum: u32) -> Result<u32, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let current = outer_bounds(window)?;
    let inner = window.inner_size().map_err(|e| e.to_string())?;
    let frame = (current.width.saturating_sub(inner.width), current.height.saturating_sub(inner.height));
    let (rows, bounds) = resize_bounds_with_header(current, frame, scale, rows, edge, work_area(window, false)?, market_height(window));
    if rows < minimum { return Err("空间不足：请先取消部分固定关注，至少保留一行轮播".into()); }
    apply_bounds(window, bounds, frame)?;
    Ok(rows)
}

pub fn reserve_ticker_rows(app: &tauri::AppHandle, db: &Database, pinned_count: usize) -> Result<(), String> {
    let minimum = (pinned_count as u32 + 1).max(2);
    if minimum > 30 { return Err("固定关注最多 29 只，需保留一行轮播".into()); }
    let window = app.get_webview_window("ticker").ok_or("Ticker window not found")?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let inner = window.inner_size().map_err(|e| e.to_string())?;
    let current_rows = ((inner.height as f64 / scale - 8.0 - market_height(&window) as f64) / 15.0).round().max(2.0) as u32;
    if current_rows >= minimum { return Ok(()); }
    let current = outer_bounds(&window)?;
    let area = work_area(&window, false)?;
    let (preferred, fallback) = if current.y * 2 + current.height as i32 > area.y * 2 + area.height as i32 {
        (ResizeEdge::Top, ResizeEdge::Bottom)
    } else { (ResizeEdge::Bottom, ResizeEdge::Top) };
    let rows = resize_ticker_at_least(&window, minimum, preferred, minimum)
        .or_else(|_| resize_ticker_at_least(&window, minimum, fallback, minimum))?;
    db.set_setting("ticker_rows", &rows.to_string()).map_err(|e| e.to_string())
}

/// Repair the actual position, not just the saved coordinates.
pub fn ensure_ticker_visible(window: &tauri::WebviewWindow) -> Result<(), String> {
    let current = outer_bounds(window)?;
    let safe = clamp_bounds(current, work_area(window, false)?);
    if safe != current {
        window.set_position(tauri::PhysicalPosition::new(safe.x, safe.y)).map_err(|e| e.to_string())?;
    }
    window.set_always_on_top(true).map_err(|e| e.to_string())
}

pub fn restore_ticker_position(window: &tauri::WebviewWindow, db: &Database, reset: bool) -> Result<(), String> {
    if !reset {
        let saved_x = db.get_setting("ticker_x").ok().flatten().and_then(|v| v.parse::<i32>().ok());
        let saved_y = db.get_setting("ticker_y").ok().flatten().and_then(|v| v.parse::<i32>().ok());
        if let (Some(x), Some(y)) = (saved_x, saved_y) {
            window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
            return ensure_ticker_visible(window);
        }
    }
    let area = work_area(window, true)?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let safe = clamp_bounds(Bounds {
        x: area.x + area.width as i32 - size.width as i32,
        y: area.y + area.height as i32 - size.height as i32,
        width: size.width, height: size.height,
    }, area);
    window.set_position(tauri::PhysicalPosition::new(safe.x, safe.y)).map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_ticker_rows(app: tauri::AppHandle, db: tauri::State<'_, Arc<Database>>, rows: u32, edge: Option<ResizeEdge>) -> Result<u32, String> {
    let window = app.get_webview_window("ticker").ok_or("Ticker window not found")?;
    let minimum = (db.get_watchlist().map_err(|e| e.to_string())?.iter()
        .filter(|i| i.ticker_enabled && i.ticker_pinned).count() as u32 + 1).max(2);
    let rows = resize_ticker_at_least(&window, rows.max(minimum), edge.unwrap_or_default(), minimum)?;
    db.set_setting("ticker_rows", &rows.to_string()).map_err(|e| e.to_string())?;
    Ok(rows)
}


#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
