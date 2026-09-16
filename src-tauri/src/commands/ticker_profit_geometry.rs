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

pub fn anchor_right(current: Bounds, area: Bounds, previous: bool) -> bool {
    let center = current.x as i64 * 2 + current.width as i64;
    let middle = area.x as i64 * 2 + area.width as i64;
    if center == middle { previous } else { center > middle }
}

pub fn width_bounds(current: Bounds, width: u32, right: bool, area: Bounds) -> Bounds {
    clamp_bounds(Bounds {
        x: if right { current.x + current.width as i32 - width as i32 } else { current.x },
        width, ..current
    }, area)
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profit_toggle_preserves_selected_edge_and_round_trips() {
        let area = Bounds { x: -1920, y: 0, width: 1920, height: 1040 };
        for x in [-1800, -350] {
            let original = Bounds { x, y: 800, width: 343, height: 76 };
            let right = anchor_right(original, area, false);
            let hidden = width_bounds(original, 251, right, area);
            assert_eq!(hidden.y, original.y);
            assert_eq!(hidden.height, original.height);
            if right { assert_eq!(hidden.x + hidden.width as i32, original.x + original.width as i32); }
            else { assert_eq!(hidden.x, original.x); }
            assert_eq!(width_bounds(hidden, original.width, right, area), original);
        }
    }
}
