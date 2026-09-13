//! A minimal HTTP/1.1 client: one `GET http://host[:port]/path`, that is all
//! this application needs. See Cargo.toml for why there is no HTTP crate here.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// `Ok((status, body))` when the request completed; `Err` for socket problems.
pub async fn get(url: &str, timeout: Duration) -> Result<(u16, String), String> {
    let host_and_path = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("only http:// URLs are supported: {}", url))?;

    let (host_port, path) = match host_and_path.find('/') {
        Some(i) => (&host_and_path[..i], &host_and_path[i..]),
        None => (host_and_path, "/"),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|e| e.to_string())?),
        None => (host_port, 80),
    };

    let request = async {
        let mut stream = tokio::net::TcpStream::connect((host, port))
            .await
            .map_err(|e| format!("connect {}:{} failed: {}", host, port, e))?;
        let head = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: log-output\r\n\r\n",
            path, host
        );
        stream
            .write_all(head.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&raw).to_string();

        let status: u16 = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| "no HTTP status line in the response".to_string())?;
        let body = text
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string();
        Ok::<(u16, String), String>((status, body))
    };

    match tokio::time::timeout(timeout, request).await {
        Ok(result) => result,
        Err(_) => Err(format!("no answer from {} within {:?}", url, timeout)),
    }
}