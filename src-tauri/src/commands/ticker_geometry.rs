#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResizeEdge {
    Top,
    #[default]
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn clamp_bounds(mut bounds: Bounds, area: Bounds) -> Bounds {
    bounds.x = bounds.x.clamp(area.x, (area.x + area.width as i32 - bounds.width as i32).max(area.x));
    bounds.y = bounds.y.clamp(area.y, (area.y + area.height as i32 - bounds.height as i32).max(area.y));
    bounds
}

#[cfg(test)]
pub fn resize_bounds(current: Bounds, frame: (u32, u32), scale: f64, rows: u32, edge: ResizeEdge, area: Bounds) -> (u32, Bounds) {
    resize_bounds_with_header(current, frame, scale, rows, edge, area, 0)
}

pub fn resize_bounds_with_header(current: Bounds, frame: (u32, u32), scale: f64, rows: u32, edge: ResizeEdge, area: Bounds, header: u32) -> (u32, Bounds) {
    // Both the anchor and the requested size are OUTER physical pixels.
    let current = clamp_bounds(current, area);
    let available = match edge {
        ResizeEdge::Top => current.y + current.height as i32 - area.y,
        ResizeEdge::Bottom => area.y + area.height as i32 - current.y,
    };
    let max_rows = (((available as f64 - frame.1 as f64) / scale - 8.0 - header as f64) / 15.0)
        .floor().clamp(2.0, 30.0) as u32;
    let rows = rows.clamp(2, max_rows);
    let height = ((8 + rows * 15 + header) as f64 * scale).round() as u32 + frame.1;
    let bounds = Bounds {
        x: current.x,
        y: match edge {
            ResizeEdge::Top => current.y + current.height as i32 - height as i32,
            ResizeEdge::Bottom => current.y,
        },
        width: current.width,
        height,
    };
    (rows, clamp_bounds(bounds, area))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_header_preserves_stock_rows_and_fractional_dpi_anchor() {
        let area = Bounds { x: 0, y: 0, width: 1920, height: 1040 };
        let current = Bounds { x: 1400, y: 900, width: 251, height: 76 };
        for edge in [ResizeEdge::Top, ResizeEdge::Bottom] {
            let (rows, with_header) = resize_bounds_with_header(current, (18, 10), 1.25, 3, edge, area, 15);
            assert_eq!(rows, 3);
            assert_eq!(with_header.height, 95);
            let (_, restored) = resize_bounds_with_header(with_header, (18, 10), 1.25, 3, edge, area, 0);
            assert_eq!(restored, current);
        }
    }

    #[test]
    fn top_resize_keeps_outer_bottom_with_windows_frame() {
        let area = Bounds { x: 0, y: 0, width: 1920, height: 1040 };
        let mut current = Bounds { x: 1400, y: 900, width: 325, height: 91 };
        let bottom = current.y + current.height as i32;
        for rows in [6, 7, 3, 2, 8, 2, 6] {
            let (_, next) = resize_bounds(current, (15, 8), 1.0, rows, ResizeEdge::Top, area);
            assert_eq!(next.y + next.height as i32, bottom);
            assert_eq!(next.height, 8 + rows * 15 + 8);
            current = next;
        }
    }

    #[test]
    fn bottom_resize_keeps_top_at_fractional_dpi_and_limits_at_taskbar() {
        let area = Bounds { x: -1920, y: 80, width: 1920, height: 960 };
        let current = Bounds { x: -600, y: 800, width: 400, height: 56 };
        let (rows, next) = resize_bounds(current, (18, 9), 1.25, 30, ResizeEdge::Bottom, area);
        assert!(rows < 30);
        assert_eq!(next.y, 800);
        assert!(next.y + next.height as i32 <= 1040);
        let (_, smaller) = resize_bounds(next, (18, 9), 1.25, 2, ResizeEdge::Bottom, area);
        assert_eq!(smaller.y, 800);
    }

    #[test]
    fn recovery_excludes_taskbar_on_every_edge_and_handles_negative_monitor_origin() {
        let area = Bounds { x: -1840, y: 40, width: 1840, height: 1000 };
        for (x, y) in [(-1920, 0), (-20, 1000), (2000, 3000)] {
            let result = clamp_bounds(Bounds { x, y, width: 325, height: 91 }, area);
            assert!(result.x >= area.x && result.y >= area.y);
            assert!(result.x + result.width as i32 <= 0);
            assert!(result.y + result.height as i32 <= 1040);
        }
    }
}
