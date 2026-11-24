// window_bounds.rs — 行情条位置边界管理。
//
// 边界是「显示器工作区」而不是显示器矩形：Windows 任务栏、macOS 菜单栏/Dock、
// Linux 面板都会让 `work_area` 小于 `size`，按工作区钳制就同时解决了遮挡问题。
// 多显示器之间允许窗口骑在交界上，只要矩形被**所有工作区的并集**完整覆盖 ——
// 这是「能顺利跨屏」与「整条永远可见」之间的平衡。
//
// 本模块只管几何与调用时机；平台差异只在「从哪拿工作区」和「什么时候调用」。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

fn area_rect(a: &WorkArea) -> Rect {
    Rect {
        x: a.x as i64,
        y: a.y as i64,
        w: a.w as i64,
        h: a.h as i64,
    }
}

/// 把窗口矩形钳回「所有工作区的并集」内，返回位移最小的左上角。
///
/// 已完整可见时原样返回（包括骑在两块相邻显示器交界上的情况）；
/// 没有任何可行解（没有显示器，或窗口比所有工作区都大）时：
/// - 没有显示器 → `None`；
/// - 窗口过大 → 返回重叠面积最大者的左上角，至少锚在可见区域。
pub(crate) fn clamp_rect(
    pos: (i32, i32),
    size: (u32, u32),
    areas: &[WorkArea],
) -> Option<(i32, i32)> {
    if areas.is_empty() {
        return None;
    }
    let rect = Rect {
        x: pos.0 as i64,
        y: pos.1 as i64,
        w: size.0 as i64,
        h: size.1 as i64,
    };
    if rect.w <= 0 || rect.h <= 0 {
        return Some(pos);
    }
    if covered(rect, areas) {
        return Some(pos);
    }

    // 最小修正一定发生在某条显示器边界上：把候选位移限制为「0 或与某条边对齐」。
    let mut xs = vec![rect.x];
    let mut ys = vec![rect.y];
    for a in areas {
        let r = area_rect(a);
        xs.push(r.x);
        xs.push(r.x + r.w - rect.w);
        ys.push(r.y);
        ys.push(r.y + r.h - rect.h);
    }
    xs.sort_unstable();
    xs.dedup();
    ys.sort_unstable();
    ys.dedup();

    let mut best: Option<((i32, i32), i64)> = None;
    for &x in &xs {
        for &y in &ys {
            let candidate = Rect { x, y, ..rect };
            if !covered(candidate, areas) {
                continue;
            }
            let dx = x - rect.x;
            let dy = y - rect.y;
            let cost = dx * dx + dy * dy;
            if best.map_or(true, |(_, c)| cost < c) {
                best = Some(((x as i32, y as i32), cost));
            }
        }
    }
    if let Some((p, _)) = best {
        return Some(p);
    }

    let mut best_overlap = 0i64;
    let mut fallback = (areas[0].x, areas[0].y);
    for a in areas {
        let overlap = overlap_area(rect, a);
        if overlap > best_overlap {
            best_overlap = overlap;
            fallback = (a.x, a.y);
        }
    }
    Some(fallback)
}

/// 精确判断矩形是否被工作区并集完整覆盖。
///
/// 按所有 x 边界切成条带，每个条带内任取一点检查 y 区间的并集是否覆盖
/// `[top, bottom]` —— 条带内每块工作区要么整段覆盖、要么完全不覆盖，所以
/// 取条带左端点就足够，不需要采样。
fn covered(rect: Rect, areas: &[WorkArea]) -> bool {
    let left = rect.x;
    let right = rect.x + rect.w;
    let top = rect.y;
    let bottom = rect.y + rect.h;

    let mut xs = vec![left, right];
    for a in areas {
        let r = area_rect(a);
        if r.x + r.w > left && r.x < right {
            xs.push(r.x.clamp(left, right));
            xs.push((r.x + r.w).clamp(left, right));
        }
    }
    xs.sort_unstable();
    xs.dedup();

    for pair in xs.windows(2) {
        let strip_left = pair[0];
        if pair[1] <= strip_left {
            continue;
        }
        let mut spans: Vec<(i64, i64)> = areas
            .iter()
            .map(area_rect)
            .filter(|r| r.x <= strip_left && strip_left < r.x + r.w)
            .map(|r| (r.y, r.y + r.h))
            .collect();
        if spans.is_empty() {
            return false;
        }
        spans.sort_unstable();
        // cursor 式区间合并：从 top 开始逐段向前推进。完全落在 cursor 之下的
        // span 跳过；只在下一次起点已超过 cursor（且尚未到 bottom）时才是空洞。
        // 先判断 cursor >= bottom 再检查间隙，已覆盖的矩形不会被上方无关 span 判死。
        let mut cursor = top;
        for &(span_top, span_bottom) in &spans {
            if cursor >= bottom {
                break;
            }
            if span_bottom <= cursor {
                continue;
            }
            if span_top > cursor {
                return false;
            }
            cursor = cursor.max(span_bottom);
        }
        if cursor < bottom {
            return false;
        }
    }
    true
}

fn overlap_area(rect: Rect, a: &WorkArea) -> i64 {
    let r = area_rect(a);
    let left = rect.x.max(r.x);
    let right = (rect.x + rect.w).min(r.x + r.w);
    let top = rect.y.max(r.y);
    let bottom = (rect.y + rect.h).min(r.y + r.h);
    if right <= left || bottom <= top {
        0
    } else {
        (right - left) * (bottom - top)
    }
}

/// 工作区右下角、向内留 `margin` 的默认位置（首启与存档全失效时用）。
fn bottom_right_in(area: &WorkArea, size: (u32, u32), margin: i32) -> (i32, i32) {
    let max_x = area.x as i64 + area.w as i64 - size.0 as i64;
    let max_y = area.y as i64 + area.h as i64 - size.1 as i64;
    (
        (max_x - margin as i64).max(area.x as i64) as i32,
        (max_y - margin as i64).max(area.y as i64) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, w: u32, h: u32) -> WorkArea {
        WorkArea { x, y, w, h }
    }

    const SIZE: (u32, u32) = (220, 40);

    #[test]
    fn fully_visible_position_is_unchanged() {
        let areas = [area(0, 0, 1920, 1040)];
        assert_eq!(clamp_rect((100, 100), SIZE, &areas), Some((100, 100)));
    }

    #[test]
    fn clamps_at_each_edge_of_a_single_work_area() {
        let areas = [area(0, 0, 1920, 1040)];
        assert_eq!(clamp_rect((-50, 100), SIZE, &areas), Some((0, 100)));
        assert_eq!(clamp_rect((1800, 100), SIZE, &areas), Some((1700, 100)));
        assert_eq!(clamp_rect((100, -50), SIZE, &areas), Some((100, 0)));
        assert_eq!(clamp_rect((100, 1030), SIZE, &areas), Some((100, 1000)));
    }

    #[test]
    fn taskbar_offsets_shrink_the_work_area() {
        // 任务栏在底部
        let areas = [area(0, 0, 1920, 1000)];
        assert_eq!(clamp_rect((100, 1010), SIZE, &areas), Some((100, 960)));

        // 任务栏在左侧
        let areas = [area(60, 0, 1860, 1040)];
        assert_eq!(clamp_rect((10, 100), SIZE, &areas), Some((60, 100)));

        // 任务栏在顶部
        let areas = [area(0, 40, 1920, 1000)];
        assert_eq!(clamp_rect((100, 10), SIZE, &areas), Some((100, 40)));
    }

    #[test]
    fn left_secondary_monitor_with_negative_coords_is_kept() {
        let areas = [area(0, 0, 1920, 1040), area(-1920, 0, 1920, 1040)];
        assert_eq!(clamp_rect((-1900, 100), SIZE, &areas), Some((-1900, 100)));
        assert_eq!(clamp_rect((-2000, 100), SIZE, &areas), Some((-1920, 100)));
    }

    #[test]
    fn right_secondary_monitor_is_kept() {
        let areas = [area(0, 0, 1920, 1040), area(1920, 0, 1920, 1040)];
        assert_eq!(clamp_rect((2200, 100), SIZE, &areas), Some((2200, 100)));
        assert_eq!(clamp_rect((3700, 100), SIZE, &areas), Some((3620, 100)));
    }

    #[test]
    fn straddling_adjacent_monitors_is_allowed() {
        let areas = [area(0, 0, 1920, 1040), area(1920, 0, 1920, 1040)];
        // 1800..2020 跨在交界上，但每个像素都在某块工作区内
        assert_eq!(clamp_rect((1800, 100), SIZE, &areas), Some((1800, 100)));
    }

    #[test]
    fn straddling_offset_monitors_uses_the_smallest_correction() {
        // B 比 A 低 100px：交界处 x>=1920 且 y<100 的区域是空洞。
        // 往下挪 50px 比往回挪 100px 更近，所以应选 (1800, 100)。
        let areas = [area(0, 0, 1920, 1040), area(1920, 100, 1920, 940)];
        assert_eq!(clamp_rect((1800, 50), SIZE, &areas), Some((1800, 100)));
    }

    #[test]
    fn gap_between_monitors_is_not_visible_area() {
        let areas = [area(0, 0, 1000, 1000), area(1200, 0, 1000, 1000)];
        // 900..1120 有 120px 落在 1000..1200 的空洞里
        assert_eq!(clamp_rect((900, 100), SIZE, &areas), Some((780, 100)));
    }

    #[test]
    fn stacked_monitors_with_gap_keep_window_in_place() {
        // 上下叠放、x 区间相同但 y 之间有 120px 空洞（1080..1200）。
        let areas = [area(0, 0, 1920, 1080), area(0, 1200, 1920, 1080)];

        // 整条落在上屏内：必须原样返回，不能因为下方 span 与上方 span 的间隙被判死。
        assert_eq!(clamp_rect((100, 1300), SIZE, &areas), Some((100, 1300)));

        // 整条落在空洞里（1100..1140）：必须被拉回某块屏内，且落点完整可见。
        // 往上 60px（到 1040）比往下 100px（到 1200）更近。
        assert_eq!(clamp_rect((100, 1100), SIZE, &areas), Some((100, 1040)));
    }

    #[test]
    fn no_monitors_returns_none() {
        assert_eq!(clamp_rect((0, 0), SIZE, &[]), None);
    }

    #[test]
    fn oversized_window_falls_back_to_area_origin() {
        let areas = [area(0, 0, 100, 100)];
        assert_eq!(clamp_rect((500, 500), (220, 40), &areas), Some((0, 0)));
    }

    #[test]
    fn bottom_right_default_stays_inside_with_margin() {
        let a = area(0, 0, 1920, 1040);
        assert_eq!(bottom_right_in(&a, SIZE, 10), (1690, 990));
    }
}
