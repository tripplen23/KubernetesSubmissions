use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{Html, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::{env, net::SocketAddr};

/// Where the hourly image is cached. The file lives on the
/// PersistentVolume mounted at /usr/src/app/files, so it survives pod
/// restarts and the Lorem Picsum API isn't needed on every request.
const IMAGE_PATH: &str = "/usr/src/app/files/image.jpg";
/// Lorem Picsum random image endpoint.
const IMAGE_URL: &str = "https://picsum.photos/1200";
/// The image stays the same for 10 minutes.
const MAX_AGE_SECS: u64 = 600;

#[derive(Serialize)]
struct Status {
    status: String,
}

/// GET / — small HTML landing page that shows the hourly image.
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
      img.hourly {
        max-width: 100%; height: auto; border-radius: 8px;
        margin: 1rem 0; display: block;
      }
      ul { padding-left: 1.25rem; }
    </style>
  </head>
  <body>
    <h1>Todo App</h1>
    <p class="muted">DevOps with Kubernetes &mdash; Exercise 1.12</p>
    <p>This page is served by the <code>todo-app</code> pod running in the
       Kubernetes cluster, reached via an <code>Ingress</code> (Traefik).</p>

    <img class="hourly" src="/image" alt="Hourly picture from Lorem Picsum" />

    <h2>Available endpoints</h2>
    <ul>
      <li><code>GET /</code> &mdash; this page</li>
      <li><code>GET /image</code> &mdash; the hourly picture (cached in a PersistentVolume)</li>
      <li><code>GET /api/health</code> &mdash; <code>{"status":"ok"}</code></li>
      <li><code>GET /api/todos</code> &mdash; empty list (placeholder)</li>
      <li><code>GET /shutdown</code> &mdash; exits the process (for testing persistence)</li>
    </ul>
  </body>
</html>"#,
    )
}

/// GET /image — serves the cached picture if it is younger than 10
/// minutes; otherwise fetches a fresh one from Lorem Picsum, stores it
/// on the PersistentVolume and serves it.
async fn image() -> Response {
    let path = env::var("IMAGE_PATH").unwrap_or_else(|_| IMAGE_PATH.to_string());

    // 1) Fresh cache hit? Serve it without touching the network.
    if let Ok(meta) = tokio::fs::metadata(&path).await {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = modified.elapsed() {
                if age.as_secs() < MAX_AGE_SECS {
                    if let Ok(bytes) = tokio::fs::read(&path).await {
                        println!("Serving cached image (age {}s)", age.as_secs());
                        return image_response(bytes);
                    }
                } else {
                    println!("Cached image expired (age {}s) — fetching new", age.as_secs());
                }
            }
        }
    }

    // 2) No cache or expired → fetch a new random picture.
    match reqwest::get(IMAGE_URL).await {
        Ok(resp) => match resp.bytes().await {
            Ok(bytes) => {
                let bytes = bytes.to_vec();
                // Cache on the PV so the API isn't needed for the next 10
                // minutes and the image survives a container crash.
                if let Err(e) = tokio::fs::write(&path, &bytes).await {
                    eprintln!("Failed to cache image: {}", e);
                } else {
                    println!("Cached new image to {}", path);
                }
                image_response(bytes)
            }
            Err(e) => {
                eprintln!("Failed to read image body: {}", e);
                error_response()
            }
        },
        Err(e) => {
            eprintln!("Failed to fetch image: {}", e);
            error_response()
        }
    }
}

/// GET /shutdown — exits the process so you can test what happens when
/// the container shuts down (the image must survive, it is on the PV).
async fn shutdown() -> &'static str {
    println!("Shutting down on request (exercise 1.12 test)");
    std::process::exit(0);
}

fn image_response(bytes: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .body(Body::from(bytes))
        .unwrap()
}

fn error_response() -> Response {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Body::from("Failed to fetch image"))
        .unwrap()
}

async fn health() -> Json<Status> {
    Json(Status {
        status: "ok".to_string(),
    })
}

#[derive(Serialize)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
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
        .route("/image", get(image))
        .route("/shutdown", get(shutdown))
        .route("/api/health", get(health))
        .route("/api/todos", get(list_todos));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
