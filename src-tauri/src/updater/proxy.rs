use reqwest::{Client, Proxy};
use std::env;
use std::net::TcpStream;
use std::time::Duration;

/// Known local proxy ports — ordered by priority (most common first)
const PROXY_PORTS: &[(u16, &str)] = &[
    (7890, "Clash"),
    (10809, "Clash Meta"),
    (7891, "Clash (alt)"),
    (1080, "V2Ray/SOCKS5"),
    (10808, "V2Ray HTTP"),
    (8118, "Privoxy"),
    (8080, "Generic HTTP"),
];

/// Build a `reqwest::Client` with proxy auto-detection.
/// Detection order: system env vars → local proxy ports → direct
pub fn build_proxied_client() -> Client {
    let builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10));

    // 1. Try system proxy
    if let Some(proxy_url) = system_proxy_url() {
        if let Ok(proxy) = Proxy::all(&proxy_url) {
            log::info!("[updater] Using system proxy: {}", proxy_url);
            return builder.proxy(proxy).build()
                .expect("Failed to build reqwest client with system proxy");
        }
    }

    // 2. Scan local proxy ports
    for &(port, name) in PROXY_PORTS {
        if is_port_open(port) {
            let proxy_url = format!("http://127.0.0.1:{}", port);
            if let Ok(proxy) = Proxy::all(&proxy_url) {
                log::info!("[updater] Using local proxy ({}) at {}", name, proxy_url);
                return builder.proxy(proxy).build()
                    .expect("Failed to build reqwest client with local proxy");
            }
        }
    }

    // 3. Direct connection
    log::info!("[updater] No proxy detected, using direct connection");
    builder.build().expect("Failed to build reqwest client")
}

/// Get system proxy URL from environment variables
fn system_proxy_url() -> Option<String> {
    env::var("HTTPS_PROXY")
        .or_else(|_| env::var("https_proxy"))
        .or_else(|_| env::var("HTTP_PROXY"))
        .or_else(|_| env::var("http_proxy"))
        .ok()
        .filter(|s| !s.is_empty())
}

/// Check if a TCP port is open on localhost (1s timeout)
fn is_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_secs(1),
    )
    .is_ok()
}

/// Detect the first available proxy URL (for diagnostics/logging)
pub fn detect_proxy_url() -> Option<String> {
    system_proxy_url().or_else(|| {
        PROXY_PORTS.iter().find_map(|&(port, name)| {
            if is_port_open(port) {
                Some(format!("http://127.0.0.1:{} ({})", port, name))
            } else {
                None
            }
        })
    })
}
