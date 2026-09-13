mod http;

use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Router,
};
use chrono::Utc;
use std::env;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;

/// log-output — a two-container application: one image, two roles.
///
/// The role is chosen with the `ROLE` environment variable (the manifests run
/// the same image twice, overriding the command):
///
///   ROLE=writer (default) — appends `<rfc3339 timestamp>: <uuid>` to the log
///                           file every 5 seconds (LOG_FILE, WRITE_INTERVAL_MS)
///   ROLE=server           — serves that file together with the current pong
///                           count from the ping-pong application:
///                             GET /        → the file + ping-pong's answer
///                             GET /healthz → 200 only while ping-pong answers;
///                                            500 while it does not
///
/// The two containers share the file through an `emptyDir` volume, which is the
/// "two containers in one pod" part of the exercise.
const DEFAULT_LOG_FILE: &str = "/usr/src/app/files/log.txt";
const DEFAULT_PINGPONG_URL: &str = "http://pingpong-svc";
const PINGPONG_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() {
    match env::var("ROLE").unwrap_or_else(|_| "writer".to_string()).as_str() {
        "server" => run_server().await,
        _ => run_writer().await,
    }
}

async fn run_writer() {
    let path = env::var("LOG_FILE").unwrap_or_else(|_| DEFAULT_LOG_FILE.to_string());
    let interval_ms: u64 = env::var("WRITE_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);

    println!(
        "log-output writer writing to {} every {} ms",
        path, interval_ms
    );

    loop {
        let line = format!("{}: {}\n", Utc::now().to_rfc3339(), Uuid::new_v4());
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut file) => match file.write_all(line.as_bytes()) {
                Ok(_) => println!("wrote {}", line.trim()),
                Err(e) => eprintln!("log-output writer: cannot write {}: {}", path, e),
            },
            Err(e) => eprintln!("log-output writer: cannot open {}: {}", path, e),
        }
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
}

#[derive(Clone)]
struct ServerState {
    log_file: String,
    pingpong_url: String,
}

async fn run_server() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3541".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let state = ServerState {
        log_file: env::var("LOG_FILE").unwrap_or_else(|_| DEFAULT_LOG_FILE.to_string()),
        pingpong_url: env::var("PINGPONG_URL").unwrap_or_else(|_| DEFAULT_PINGPONG_URL.to_string()),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("log-output server listening on port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Ask the ping-pong application; `Ok((true, body))` when it answered 2xx.
async fn ask_pingpong(state: &ServerState, path: &str) -> Result<(bool, String), String> {
    let url = format!("{}{}", state.pingpong_url.trim_end_matches('/'), path);
    let (status, body) = http::get(&url, PINGPONG_TIMEOUT).await?;
    Ok(((200..300).contains(&status), body))
}

/// GET /healthz → ready only when the ping-pong application answers.
async fn healthz(State(state): State<ServerState>) -> (StatusCode, String) {
    let (ok, detail) = match ask_pingpong(&state, "/healthz").await {
        Ok((ok, body)) => (ok, format!("ping-pong answered {}", body)),
        Err(e) => (false, e),
    };
    println!(
        "{} /healthz -> {} ({})",
        Utc::now().to_rfc3339(),
        if ok { 200 } else { 500 },
        detail
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

/// GET / → the writer's file plus the current pong count.
async fn root(State(state): State<ServerState>) -> (StatusCode, String) {
    let file = std::fs::read_to_string(&state.log_file)
        .unwrap_or_else(|e| format!("(cannot read {}: {})\n", state.log_file, e));

    let pingpong = match ask_pingpong(&state, "/").await {
        Ok((_, body)) => body,
        Err(e) => format!("unreachable: {}", e),
    };

    println!("{} GET / -> 200", Utc::now().to_rfc3339());
    (
        StatusCode::OK,
        format!("{}\nping-pong answered: {}\n", file.trim_end(), pingpong),
    )
}