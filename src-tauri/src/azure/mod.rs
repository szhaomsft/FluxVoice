pub mod speech;
pub mod openai;

use std::sync::OnceLock;
use std::time::Duration;

/// Global HTTP client for connection pooling (HTTP Keep-Alive)
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(2)
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client")
    })
}
