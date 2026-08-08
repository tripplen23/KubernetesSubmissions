use axum::{routing::get, Router};
use std::env;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

/// In-memory request counter. Resets when the process restarts
/// (pod restart / new replica) — that's expected for this exercise.
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// GET /pingpong → "pong 0", "pong 1", "pong 2", ...
///
/// The counter is incremented on every request so you can see how
/// many requests the pod has served since it started.
async fn pong() -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("pong {}", n)
}

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
