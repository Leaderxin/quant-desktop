use tauri::{Manager, Emitter};
use std::sync::Arc;
use crate::db::Database;

use super::ticker_profit_geometry::{Bounds, anchor_right, width_bounds};

pub struct TickerProfitMenu(pub tauri::menu::CheckMenuItem<tauri::Wry>);

#[derive(serde::Serialize, serde::Deserialize)]
struct WidthAnchor {
    x: i32,
    width: u32,
    area: Bounds,
    right: bool,
}

pub fn profit_visible(db: &Database) -> bool {
    match db.get_setting("ticker_profit_visible") {
        Ok(value) => value.as_deref() != Some("0"),
        Err(_) => false,
    }
}

pub fn apply_profit_visibility(app: &tauri::AppHandle, db: &Database, visible: bool, restoring: bool) -> Result<(), String> {
    let window = app.get_webview_window("ticker").ok_or("Ticker window not found")?;
    let current = outer_bounds(&window)?;
    let inner = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let area = work_area(&window, false)?;
    let previous = db.get_setting("ticker_width_anchor").ok().flatten()
        .and_then(|v| serde_json::from_str::<WidthAnchor>(&v).ok());
    let right = match previous {
        Some(ref old) if old.area == area && (restoring || (old.x == current.x && old.width == current.width)) => old.right,
        ref old => anchor_right(current, area, old.as_ref().map(|a| a.right).unwrap_or(false)),
    };
    let frame = (current.width.saturating_sub(inner.width), current.height.saturating_sub(inner.height));
    let width = ((if visible { 260.0 } else { 186.0 }) * scale).round() as u32 + frame.0;
    let bounds = width_bounds(current, width, right, area);
    // Persist and notify hiding first; a failed native resize must not reveal amounts.
    db.set_setting("ticker_profit_visible", if visible { "1" } else { "0" }).map_err(|e| e.to_string())?;
    if !visible { let _ = app.emit("ticker-profit-changed", false); }
    if let Some(menu) = app.try_state::<TickerProfitMenu>() { let _ = menu.0.set_checked(visible); }
    if let Err(error) = apply_bounds(&window, bounds, frame) {
        if visible {
            let _ = db.set_setting("ticker_profit_visible", "0");
            let _ = app.emit("ticker-profit-changed", false);
            if let Some(menu) = app.try_state::<TickerProfitMenu>() { let _ = menu.0.set_checked(false); }
        }
        return Err(error);
    }
    let anchor = WidthAnchor { x: bounds.x, width: bounds.width, area, right };
    db.set_setting("ticker_width_anchor", &serde_json::to_string(&anchor).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if visible { let _ = app.emit("ticker-profit-changed", true); }
    Ok(())
}

#[tauri::command]
pub fn get_ticker_profit_visible(db: tauri::State<'_, Arc<Database>>) -> bool { profit_visible(&db) }

#[tauri::command]
pub fn toggle_ticker_profit(app: tauri::AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<bool, String> {
    let visible = !profit_visible(&db);
    apply_profit_visibility(&app, &db, visible, false)?;
    Ok(visible)
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
        unsafe { crate::windows_util::set_profit_bounds(hwnd.0 as isize, bounds.x, bounds.y, bounds.width, bounds.height) }
    }
    #[cfg(not(target_os = "windows"))]
    {
        window.set_size(tauri::PhysicalSize::new(bounds.width.saturating_sub(frame.0), bounds.height.saturating_sub(frame.1)))
            .map_err(|e| e.to_string())?;
        window.set_position(tauri::PhysicalPosition::new(bounds.x, bounds.y)).map_err(|e| e.to_string())
    }
}


