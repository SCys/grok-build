//! Short HTTPS probes for startup: skip grok.com work when the host is firewalled.
//!
//! Uses reqwest so `https_proxy` / `HTTPS_PROXY` are honored (raw TCP is not).
//! Any HTTP response (including 401/404) means the origin is reachable.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::blocking::Client;

/// Bound for DNS + connect + one response. Keep well under the TUI connect budget.
pub const STARTUP_REACHABILITY_TIMEOUT: Duration = Duration::from_millis(500);

const AUTH_ISSUER_DEFAULT: &str = "https://auth.x.ai";
const CLI_CHAT_PROXY_DEFAULT: &str = "https://cli-chat-proxy.grok.com/v1";

/// True when `url`'s origin answers within [`STARTUP_REACHABILITY_TIMEOUT`].
pub fn https_url_reachable(url: &str) -> bool {
    let url = url.to_string();
    // Run probe in an isolated OS thread so reqwest::blocking::Client's
    // internal runtime setup/drop never collides with an outer Tokio context.
    std::thread::spawn(move || {
        let Ok(client) = Client::builder()
            .connect_timeout(STARTUP_REACHABILITY_TIMEOUT)
            .timeout(STARTUP_REACHABILITY_TIMEOUT)
            .build()
        else {
            return false;
        };
        match client.get(&url).send() {
            Ok(_) => true,
            Err(err) => err.status().is_some(),
        }
    })
    .join()
    .unwrap_or(false)
}

fn cached(cell: &OnceLock<bool>, url: &str, label: &str) -> bool {
    *cell.get_or_init(|| {
        let ok = https_url_reachable(url);
        tracing::info!(url, ok, label, "startup reachability");
        ok
    })
}

/// cli-chat-proxy (settings + model catalog). Cached for the process.
pub fn cli_chat_proxy_reachable(base_url: &str) -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    let url = if base_url.is_empty() {
        CLI_CHAT_PROXY_DEFAULT
    } else {
        base_url
    };
    cached(&OK, url, "cli-chat-proxy")
}

/// OIDC/OAuth issuer used by interactive login. Cached for the process.
pub fn auth_issuer_reachable(issuer: &str) -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    let url = if issuer.is_empty() {
        AUTH_ISSUER_DEFAULT
    } else {
        issuer
    };
    cached(&OK, url, "auth-issuer")
}

/// Production grok.com login issuer (`GROK_OIDC_ISSUER` override, else auth.x.ai).
pub fn default_auth_issuer_url() -> String {
    std::env::var("GROK_OIDC_ISSUER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| AUTH_ISSUER_DEFAULT.to_string())
}

/// Production cli-chat-proxy (`GROK_CLI_CHAT_PROXY_BASE_URL` override, else default).
pub fn default_cli_chat_proxy_url() -> String {
    std::env::var("GROK_CLI_CHAT_PROXY_BASE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| CLI_CHAT_PROXY_DEFAULT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_local_port_is_unreachable_quickly() {
        let started = std::time::Instant::now();
        assert!(!https_url_reachable("https://127.0.0.1:1/"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "probe must fail fast on a closed port"
        );
    }

    #[test]
    fn default_urls_are_https() {
        assert!(default_auth_issuer_url().starts_with("https://"));
        assert!(default_cli_chat_proxy_url().starts_with("https://"));
    }
}
