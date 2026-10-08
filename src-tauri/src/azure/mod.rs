pub mod openai;
pub mod speech;

use std::sync::OnceLock;
use std::time::Duration;

/// Global HTTP client for connection pooling (HTTP Keep-Alive)
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn get_http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Duration::from_secs(600))
            .pool_max_idle_per_host(2)
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client")
    })
}

pub async fn warm_connections(speech_region: &str, openai_endpoint: Option<&str>) {
    let speech_url = format!("https://{}.api.cognitive.microsoft.com/", speech_region);
    let speech = warm_connection("speech", &speech_url);
    let openai = async {
        if let Some(endpoint) = openai_endpoint {
            warm_connection("openai", endpoint).await;
        }
    };
    tokio::join!(speech, openai);
}

async fn warm_connection(service: &str, endpoint: &str) {
    let mut url = match reqwest::Url::parse(endpoint) {
        Ok(url)
            if matches!(url.scheme(), "https" | "http")
                && url.username().is_empty()
                && url.password().is_none() =>
        {
            url
        }
        _ => {
            println!(
                "[latency] connection_warmup service={} failed=invalid_endpoint",
                service
            );
            log::warn!("Cannot warm {} connection: invalid endpoint", service);
            return;
        }
    };
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);

    let started = std::time::Instant::now();
    // A HEAD response (including 401/404/405) establishes a connection without inference.
    match get_http_client()
        .head(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => {
            println!(
                "[latency] connection_warmup service={} duration_ms={:.1} status={}",
                service,
                started.elapsed().as_secs_f64() * 1000.0,
                response.status().as_u16()
            );
        }
        Err(error) => {
            println!(
                "[latency] connection_warmup service={} duration_ms={:.1} success=false timeout={}",
                service,
                started.elapsed().as_secs_f64() * 1000.0,
                error.is_timeout()
            );
            log::warn!(
                "Failed to warm {} connection (timeout: {})",
                service,
                error.is_timeout()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn warmup_sends_no_credentials_and_reuses_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            for expected_method in ["HEAD", "GET"] {
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut buffer = [0; 1024];
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert!(count > 0, "Client closed the warmed connection");
                    request.extend_from_slice(&buffer[..count]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with(&format!("{} / HTTP/1.1\r\n", expected_method)));
                assert!(!request.to_lowercase().contains("authorization:"));
                assert!(!request.to_lowercase().contains("api-key:"));
                assert!(!request.contains("secret"));
                stream.write_all(
                        b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n",
                    ).await.unwrap();
            }
        });

        tokio::time::timeout(Duration::from_secs(10), async {
            warm_connection("test", &format!("{}/unused?secret=unused", endpoint)).await;
            let response = get_http_client().get(&endpoint).send().await.unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
            response.bytes().await.unwrap();
            server.await.unwrap();
        })
        .await
        .expect("Warmup did not reuse the HTTP connection");
    }
}
