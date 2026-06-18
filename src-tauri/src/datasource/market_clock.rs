use chrono::{Local, Datelike, NaiveTime, Weekday};

/// A-share market trading session
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketSession {
    /// Before 9:30 AM — pre-market
    PreOpen,
    /// 9:30–11:30 AM — morning trading
    MorningTrade,
    /// 11:30 AM–1:00 PM — lunch break
    LunchBreak,
    /// 1:00–3:00 PM — afternoon trading
    AfternoonTrade,
    /// After 3:00 PM or weekend/holiday — closed
    Closed,
}

impl MarketSession {
    /// Determine the current A-share market session (China Standard Time / UTC+8)
    pub fn current() -> Self {
        let now = Local::now();

        // Check weekend
        match now.weekday() {
            Weekday::Sat | Weekday::Sun => return Self::Closed,
            _ => {}
        }

        let time = now.time();

        let morning_start = NaiveTime::from_hms_opt(9, 30, 0).expect("valid time constant");
        let morning_end = NaiveTime::from_hms_opt(11, 30, 0).expect("valid time constant");
        let afternoon_start = NaiveTime::from_hms_opt(13, 0, 0).expect("valid time constant");
        let afternoon_end = NaiveTime::from_hms_opt(15, 0, 0).expect("valid time constant");

        if time < morning_start {
            Self::PreOpen
        } else if time < morning_end {
            Self::MorningTrade
        } else if time < afternoon_start {
            Self::LunchBreak
        } else if time < afternoon_end {
            Self::AfternoonTrade
        } else {
            Self::Closed
        }
    }

    /// Recommended polling interval in seconds for this session
    pub fn recommended_interval(&self) -> u64 {
        match self {
            Self::MorningTrade | Self::AfternoonTrade => 2,
            Self::PreOpen => 5,
            Self::LunchBreak => 10,
            Self::Closed => 30,
        }
    }

    /// Human-readable session name
    pub fn name(&self) -> &str {
        match self {
            Self::PreOpen => "盘前",
            Self::MorningTrade => "早盘",
            Self::LunchBreak => "午休",
            Self::AfternoonTrade => "午盘",
            Self::Closed => "休市",
        }
    }
}
