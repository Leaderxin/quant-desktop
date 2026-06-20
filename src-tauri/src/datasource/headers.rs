use std::sync::atomic::{AtomicUsize, Ordering};

/// Pool of realistic browser User-Agent strings (Windows Chrome/Edge/Firefox)
const USER_AGENTS: &[&str] = &[
    // Chrome 131 on Windows 10
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    // Chrome 130 on Windows 10
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    // Edge 131 on Windows 10
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
    // Chrome 129 on Windows 11
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
    // Firefox 133 on Windows 10
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    // Chrome 128 on Windows 10
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
    // Edge 130 on Windows 10
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
];

/// Rotating counter for User-Agent selection
static UA_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Get a User-Agent string, rotating through the pool on each call.
pub fn pick_user_agent() -> &'static str {
    let idx = UA_COUNTER.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[idx]
}

/// Decorate a request with anti-blocking headers.
///
/// Adds: rotating User-Agent, Referer, Connection keep-alive.
/// Intentionally omits Accept / Accept-Encoding / Cache-Control —
/// these can break JSON APIs (Tencent kline etc.) that don't expect
/// browser-style content negotiation.
pub fn with_browser_headers(
    req: reqwest::RequestBuilder,
    referer: &str,
) -> reqwest::RequestBuilder {
    req.header("User-Agent", pick_user_agent())
        .header("Referer", referer)
        .header("Connection", "keep-alive")
}
