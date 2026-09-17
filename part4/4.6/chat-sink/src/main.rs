use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// Everything the sink remembers. It is a *stand-in* for the chat service: the
/// exercise lets you pick Discord, Telegram, Slack or "Generic", and the Generic
/// option is a URL that receives `{"user": "bot", "message": "..."}`. This service
/// is that URL, running inside the cluster — which also makes the interesting
/// question ("did a message arrive once, or six times?") countable.
#[derive(Clone)]
struct AppState {
    seen: Arc<Mutex<Vec<Value>>>,
}

/// The body the broadcaster sends. A chat service does not get to choose the
/// format it is offered, so the sink does not either: it records whatever JSON
/// arrives, and reports on the fields the known shapes use.
#[derive(Serialize)]
struct Stats {
    count: usize,
    messages: Vec<Value>,
}

#[derive(Serialize)]
struct Ack {
    received: usize,
}

/// POST / — the webhook. Every accepted message is printed (so it is visible in
/// `kubectl logs`) and kept in memory.
async fn webhook(State(state): State<AppState>, Json(msg): Json<Value>) -> Json<Ack> {
    let mut seen = state.seen.lock().unwrap();
    let n = seen.len() + 1;
    let who = msg.get("user").and_then(|v| v.as_str()).unwrap_or("bot");
    let text = msg
        .get("message")
        .or_else(|| msg.get("content"))
        .or_else(|| msg.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("(unreadable payload)");
    println!("[chat] #{n} {who}: {text}");
    seen.push(msg);
    Json(Ack { received: n })
}

/// GET /messages — everything received, in order.
async fn messages(State(state): State<AppState>) -> Json<Stats> {
    let seen = state.seen.lock().unwrap().clone();
    Json(Stats {
        count: seen.len(),
        messages: seen,
    })
}

/// GET /count — just the number. This is the line the scaling test reads.
async fn count(State(state): State<AppState>) -> String {
    format!("{}\n", state.seen.lock().unwrap().len())
}

/// POST /reset — forget everything, so the next experiment starts from zero.
async fn reset(State(state): State<AppState>) -> Json<Ack> {
    let mut seen = state.seen.lock().unwrap();
    let had = seen.len();
    seen.clear();
    println!("[chat] reset ({had} message(s) forgotten)");
    Json(Ack { received: 0 })
}

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    let app = Router::new()
        .route("/", post(webhook))
        .route("/messages", get(messages))
        .route("/count", get(count))
        .route("/reset", post(reset))
        .with_state(AppState {
            seen: Arc::new(Mutex::new(Vec::new())),
        });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("chat-sink started in port {port}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}