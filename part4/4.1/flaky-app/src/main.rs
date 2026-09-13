use axum::{
    http::StatusCode,
    routing::get,
    Router,
};
use std::env;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A deliberately unreliable HTTP server, used to practise readiness /
/// liveness probes and rolling updates.
///
/// One image, five behaviours, selected with the `FLAKY_VERSION` environment
/// variable (the deployment changes it exactly like it would change an image
/// tag, and Kubernetes starts a new rolling update):
///
///   FLAKY_VERSION=v1  always healthy            → the good version
///   FLAKY_VERSION=v2  never healthy             → /healthz always answers 500
///   FLAKY_VERSION=v3  healthy ~90% of the time  → some checks answer 500
///   FLAKY_VERSION=v4  healthy at first, then it breaks 60s after start
///   FLAKY_VERSION=v5  always healthy            → the other good version
///
/// Endpoints (port 3541):
///   GET /healthz  → 200 "ok" when healthy, 500 "unhealthy" when not
///   GET /         → 200 with the version and how long the process has been up
#[derive(Clone)]
struct AppState {
    version: String,
    started: SystemTime,
}

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3541".to_string())
        .parse()
        .expect("PORT must be a valid number");
    let version = env::var("FLAKY_VERSION").unwrap_or_else(|_| "v1".to_string());

    let state = AppState {
        version: version.clone(),
        started: SystemTime::now(),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("flaky-app {} listening on port {}", version, port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Seconds since the process started.
fn age_seconds(state: &AppState) -> u64 {
    state
        .started
        .elapsed()
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

/// Is this version healthy *right now*?
fn healthy(state: &AppState) -> bool {
    match state.version.as_str() {
        "v2" => false,
        // "roughly one check in ten" — no rand dependency needed: the
        // nanosecond clock is enough to make this look random to a probe.
        "v3" => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() % 10 != 0)
            .unwrap_or(true),
        // works for a minute, then broken until the container is restarted
        "v4" => age_seconds(state) < 60,
        _ => true,
    }
}

async fn healthz(axum::extract::State(state): axum::extract::State<AppState>) -> (StatusCode, String) {
    let ok = healthy(&state);
    println!(
        "{} {} /healthz -> {} (age {}s)",
        timestamp(),
        state.version,
        if ok { 200 } else { 500 },
        age_seconds(&state)
    );
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        },
        if ok { "ok".to_string() } else { "unhealthy".to_string() },
    )
}

async fn root(axum::extract::State(state): axum::extract::State<AppState>) -> (StatusCode, String) {
    let age = age_seconds(&state);
    println!("{} {} GET / -> 200", timestamp(), state.version);
    (
        StatusCode::OK,
        format!(
            "flaky-app {} — up {}s — healthy={}\n",
            state.version,
            age,
            healthy(&state)
        ),
    )
}

/// `2026-09-12T18:00:00.123456789+00:00` — the same format the other apps log.
fn timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}
