use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::env;
use std::net::SocketAddr;

#[derive(Serialize)]
struct Status {
    status: String,
}

#[derive(Serialize)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

async fn ping() -> &'static str {
    "pong"
}

async fn health() -> Json<Status> {
    Json(Status {
        status: "ok".to_string(),
    })
}

async fn list_todos() -> Json<Vec<Todo>> {
    // Placeholder — will connect to a database in later exercises
    Json(vec![])
}

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let app = Router::new()
        .route("/", get(ping))
        .route("/api/health", get(health))
        .route("/api/todos", get(list_todos));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
