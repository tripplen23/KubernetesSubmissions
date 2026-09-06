use axum::{routing::get, Router};
use chrono::Utc;
use std::env;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;

/// Where the writer appends timestamp lines.
const DEFAULT_FILE_PATH: &str = "/usr/src/app/files/timestamp.txt";
/// HTTP endpoint of the Ping pong app (via its Kubernetes Service).
const DEFAULT_PINGS_URL: &str = "http://ping-pong-svc:3000/pongs";

#[tokio::main]
async fn main() {
    let role = env::var("ROLE").unwrap_or_else(|_| "writer".to_string());
    match role.as_str() {
        "reader" => run_reader().await,
        _ => run_writer().await,
    }
}

/// Container 1 — "writer".
///
/// Generates a random string once at startup, then every 5 seconds
/// appends a line `<rfc3339 timestamp>: <random string>` to the shared
/// file (emptyDir).
async fn run_writer() {
    let file_path = env::var("FILE_PATH").unwrap_or_else(|_| DEFAULT_FILE_PATH.to_string());
    let random_string = Uuid::new_v4().to_string();
    println!("Writer started, random string: {}", random_string);

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let line = format!("{}: {}", Utc::now().to_rfc3339(), random_string);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .expect("open shared file");
        writeln!(file, "{}", line).expect("write line");
        println!("{}", line);
    }
}

/// Container 2 — "reader".
///
/// Serves an HTTP endpoint that shows:
///   <timestamp>: <random string>      (from timestamp.txt, emptyDir)
///   Ping / Pongs: <N>                 (HTTP GET to the Ping pong app)
///
/// The pongs count is fetched over HTTP from the Ping pong app's
/// Service (`ping-pong-svc`) instead of reading a shared file.
async fn run_reader() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");
    let file_path = env::var("FILE_PATH").unwrap_or_else(|_| DEFAULT_FILE_PATH.to_string());
    let pings_url = env::var("PINGS_URL").unwrap_or_else(|_| DEFAULT_PINGS_URL.to_string());

    let app = Router::new().route(
        "/",
        get(move || read_logs(file_path.clone(), pings_url.clone())),
    );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn read_logs(file_path: String, pings_url: String) -> String {
    let logs = match tokio::fs::read_to_string(&file_path).await {
        Ok(contents) if !contents.trim().is_empty() => contents,
        _ => "No log lines yet — the writer hasn't written anything.".to_string(),
    };

    // Fetch the pong count from the Ping pong app over HTTP.
    let pings = match reqwest::get(&pings_url).await {
        Ok(resp) => match resp.text().await {
            Ok(text) => text.trim().to_string(),
            Err(_) => "0".to_string(),
        },
        Err(e) => {
            eprintln!("Failed to reach ping-pong at {}: {}", pings_url, e);
            "0".to_string()
        }
    };

    format!("{}\nPing / Pongs: {}", logs.trim_end(), pings)
}
