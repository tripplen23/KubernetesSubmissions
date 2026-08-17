use axum::{extract::State, routing::get, Router};
use std::env;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Shared counter state. The counter lives in memory again —
/// the volume between the two applications was removed in this exercise.
#[derive(Clone)]
struct AppState {
    pongs: Arc<AtomicU64>,
}

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let state = AppState {
        pongs: Arc::new(AtomicU64::new(0)),
    };

    let app = Router::new()
        .route("/pingpong", get(pong))
        .route("/pongs", get(pongs))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET /pingpong → "pong 0", "pong 1", ...
///
/// Increments the counter and replies with the previous value.
async fn pong(State(state): State<AppState>) -> String {
    let current = state.pongs.fetch_add(1, Ordering::SeqCst);
    format!("pong {}", current)
}

/// GET /pongs → "3" (the current number of pongs, no increment)
async fn pongs(State(state): State<AppState>) -> String {
    state.pongs.load(Ordering::SeqCst).to_string()
}
