use axum::{routing::get, Router};
use std::env;
use std::net::SocketAddr;

/// File where the request counter lives. The file sits on the shared
/// PersistentVolume, so the count survives pod restarts — unlike 1.9
/// where the counter was an in-memory AtomicU64.
const DEFAULT_PINGS_FILE: &str = "/usr/src/app/files/pings.txt";

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let app = Router::new().route("/pingpong", get(pong));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET /pingpong → "pong 0", "pong 1", ...
///
/// Reads the current count from the shared file, increments it, writes
/// it back, and replies with the previous value. The count therefore
/// persists across pod restarts (the file lives on the PV).
async fn pong() -> String {
    let pings_file = env::var("PINGS_FILE").unwrap_or_else(|_| DEFAULT_PINGS_FILE.to_string());

    let current: u64 = match tokio::fs::read_to_string(&pings_file).await {
        Ok(contents) => contents.trim().parse().unwrap_or(0),
        Err(_) => 0, // file doesn't exist yet → first request
    };

    let next = current + 1;
    tokio::fs::write(&pings_file, next.to_string())
        .await
        .expect("write counter to shared volume");

    format!("pong {}", current)
}
