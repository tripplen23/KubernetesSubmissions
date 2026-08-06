use axum::{routing::get, Json, Router};
use chrono::Utc;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::interval;
use uuid::Uuid;

static RANDOM_STRING: OnceLock<String> = OnceLock::new();

#[derive(Serialize)]
struct Status {
    timestamp: String,
    random_string: String,
}

async fn status() -> Json<Status> {
    Json(Status {
        timestamp: Utc::now().to_rfc3339(),
        random_string: RANDOM_STRING
            .get()
            .expect("random string not initialized")
            .clone(),
    })
}

#[tokio::main]
async fn main() {
    RANDOM_STRING
        .set(Uuid::new_v4().to_string())
        .expect("OnceLock should be empty on first set");

    tokio::spawn(async {
        let mut tick = interval(Duration::from_secs(5));
        tick.tick().await;
        loop {
            tick.tick().await;
            let s = RANDOM_STRING.get().expect("random string missing");
            println!("{} {}", Utc::now().to_rfc3339(), s);
        }
    });

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    let app = Router::new().route("/status", get(status));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .unwrap_or_else(|e| panic!("bind 0.0.0.0:{}: {}", port, e));
    println!("Server started in port {}", port);
    axum::serve(listener, app).await.expect("server crashed");
}
