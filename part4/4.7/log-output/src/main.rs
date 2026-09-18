use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{routing::get, Router};

/// The version this pod is running. `kustomize edit set image` changes the
/// *image*, and the Deployment passes its own tag in here, so the page and the
/// logs say which release ArgoCD has rolled out.
fn version() -> String {
    std::env::var("APP_VERSION").unwrap_or_else(|_| "unknown".to_string())
}

/// A short, changing string so consecutive log lines are distinguishable
/// without pulling in a random-number crate.
fn stamp() -> (u64, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mixed = secs
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (secs, format!("{:08x}", (mixed >> 24) as u32))
}

async fn root() -> String {
    let (secs, hash) = stamp();
    format!(
        "<!doctype html><html><body style=\"font-family: monospace\">\
         <h1>Log output app</h1>\
         <p>version: <b>{}</b></p>\
         <p>request at: {} (hash {})</p>\
         <p>this pod logs one line every five seconds; the lines are in \
         <code>kubectl logs</code> and in the page below.</p>\
         </body></html>",
        version(),
        secs,
        hash
    )
}

async fn healthz() -> &'static str {
    "ok"
}

/// The point of the exercise: something that produces log output on a timer.
async fn logger_loop() {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        ticker.tick().await;
        let (secs, hash) = stamp();
        println!("[log] {} {} version={}", secs, hash, version());
    }
}

#[tokio::main]
async fn main() {
    tokio::spawn(logger_loop());

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("log-output {} listening on {}", version(), addr);

    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
