//! ping-pong — the application the canary releases.
//!
//! One image, two behaviours, chosen by `PINGPONG_MODE`:
//!
//! * `normal` (default) — answers every request and burns almost no CPU.
//! * `hog` — answers every request, and a background thread spins on arithmetic.
//!   The pod stays **healthy**: `/healthz` is 200 in both modes. This is the
//!   point of the exercise — a release can be perfectly "up" and still be a bad
//!   release, which is why the canary measures CPU instead of asking the app.
//!
//! Env: `PINGPONG_MODE` (normal|hog), `BURN_THREADS` (default 1), `PORT` (default 3541).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::Router;

#[derive(Clone)]
struct Ctx {
    mode: String,
    burn_threads: usize,
    hits: Arc<AtomicU64>,
}

#[tokio::main]
async fn main() {
    let mode = std::env::var("PINGPONG_MODE").unwrap_or_else(|_| "normal".into());
    let burn_threads: usize = std::env::var("BURN_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3541);

    // The whole difference between the two releases: a thread that never sleeps.
    if mode == "hog" {
        for i in 0..burn_threads {
            std::thread::spawn(move || {
                let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
                loop {
                    x = x
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    std::hint::black_box(x);
                }
            });
            println!("[burn] thread {i} started — this release is expensive");
        }
    }

    let ctx = Ctx {
        mode: mode.clone(),
        burn_threads,
        hits: Arc::new(AtomicU64::new(0)),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .with_state(ctx);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("cannot bind {addr}: {e}"));
    println!("ping-pong started on {addr} — mode={mode} burn_threads={burn_threads}");
    axum::serve(listener, app).await.unwrap();
}

/// Healthy in both modes — that is exactly what makes the CPU canary necessary.
async fn healthz() -> &'static str {
    "ok"
}

async fn root(State(ctx): State<Ctx>) -> String {
    let n = ctx.hits.fetch_add(1, Ordering::Relaxed) + 1;
    format!(
        "pong {n} — mode={} — burn_threads={}\n",
        ctx.mode, ctx.burn_threads
    )
}