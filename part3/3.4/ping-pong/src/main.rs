use axum::{extract::State, routing::get, Router};
use std::env;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Shared counter state. The counter lives in memory (resets on pod restart).
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

    // Exercise 3.4: thanks to the Gateway API's route rewriting, the app
    // serves its behavior at the ROOT path "/" (its natural place) instead
    // of being forced to reflect the cluster-level URL structure /pingpong.
    // The gateway rewrites /pingpong -> / before forwarding to this service.
    let app = Router::new()
        .route("/", get(pong))
        .route("/pongs", get(pongs))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET / → "pong 0", "pong 1", … (increments).
///
/// NOTE: the GKE health check also probes "/" every ~15s, so the counter
/// increments from probes too — harmless for this lab.
async fn pong(State(state): State<AppState>) -> String {
    let current = state.pongs.fetch_add(1, Ordering::SeqCst);
    format!("pong {}", current)
}

/// GET /pongs → "3" (the current number of pongs, no increment).
/// Used by the log-output reader to show "Ping / Pongs: N".
async fn pongs(State(state): State<AppState>) -> String {
    state.pongs.load(Ordering::SeqCst).to_string()
}