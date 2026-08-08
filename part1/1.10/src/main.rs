use axum::{routing::get, Router};
use chrono::Utc;
use std::env;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;

/// Where the shared emptyDir volume is mounted. Both containers mount
/// the same volume at this path, so the file written by the writer is
/// visible to the reader.
const DEFAULT_FILE_PATH: &str = "/usr/src/app/files/timestamp.txt";

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
/// appends a line `<rfc3339 timestamp> <random string>` to the shared
/// file. The file lives on the emptyDir volume shared with the reader.
async fn run_writer() {
    let file_path = env::var("FILE_PATH").unwrap_or_else(|_| DEFAULT_FILE_PATH.to_string());
    let random_string = Uuid::new_v4().to_string();
    println!("Writer started, random string: {}", random_string);

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        let line = format!("{} {}", Utc::now().to_rfc3339(), random_string);
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
/// Serves an HTTP endpoint that reads the shared file and returns its
/// contents. If the file doesn't exist yet (writer hasn't written the
/// first line), returns a placeholder message.
async fn run_reader() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");
    let file_path = env::var("FILE_PATH").unwrap_or_else(|_| DEFAULT_FILE_PATH.to_string());

    let app = Router::new().route("/", get(move || read_shared_file(file_path.clone())));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn read_shared_file(file_path: String) -> String {
    match tokio::fs::read_to_string(&file_path).await {
        Ok(contents) => {
            if contents.trim().is_empty() {
                "No log lines yet — the writer hasn't written anything.".to_string()
            } else {
                contents
            }
        }
        Err(_) => "No log lines yet — the writer hasn't written anything.".to_string(),
    }
}
