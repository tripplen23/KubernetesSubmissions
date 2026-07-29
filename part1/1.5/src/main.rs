use axum::{
    response::Html,
    routing::get,
    Json, Router,
};
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

/// GET / — small HTML landing page.
///
/// Replaces the original `pong` plain-text response with a real HTML page so
/// the project can be opened in a browser after `kubectl port-forward`.
async fn index() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Todo App</title>
    <style>
      :root { color-scheme: light dark; }
      body {
        font-family: system-ui, -apple-system, sans-serif;
        max-width: 720px; margin: 4rem auto; padding: 0 1rem;
        line-height: 1.5;
      }
      h1 { margin-bottom: 0.25rem; }
      .muted { color: #888; }
      code { background: rgba(127,127,127,.15); padding: .1em .35em; border-radius: 4px; }
      ul { padding-left: 1.25rem; }
    </style>
  </head>
  <body>
    <h1>Todo App</h1>
    <p class="muted">DevOps with Kubernetes &mdash; Exercise 1.5</p>
    <p>This page is served by the <code>todo-app</code> pod running in the
       Kubernetes cluster. The pod was reached from your browser via
       <code>kubectl port-forward</code>.</p>

    <h2>Available endpoints</h2>
    <ul>
      <li><code>GET /</code> &mdash; this page</li>
      <li><code>GET /api/health</code> &mdash; <code>{"status":"ok"}</code></li>
      <li><code>GET /api/todos</code> &mdash; empty list (placeholder)</li>
    </ul>
  </body>
</html>"#,
    )
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
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/todos", get(list_todos));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
