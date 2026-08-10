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

#[derive(Serialize)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

/// Hardcoded todos — the exercise only asks for a visible list, sending
/// and persisting comes in a later exercise.
fn hardcoded_todos() -> Vec<Todo> {
    vec![
        Todo {
            id: 1,
            title: "Learn about PersistentVolumes".to_string(),
            done: true,
        },
        Todo {
            id: 2,
            title: "Cache the hourly image on the PV".to_string(),
            done: true,
        },
        Todo {
            id: 3,
            title: "Submit exercise 1.13".to_string(),
            done: false,
        },
    ]
}

/// GET / — small HTML landing page: hourly image + todo app UI
/// (input field limited to 140 chars, a Send button and a hardcoded
/// list of todos).
async fn index() -> Html<String> {
    let todos = hardcoded_todos();
    let todo_items: String = todos
        .iter()
        .map(|t| {
            let checked = if t.done { " checked" } else { "" };
            format!(
                r#"<li><label><input type="checkbox"{checked} disabled /> <span>{}</span></label></li>"#,
                t.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n      ");

    Html(format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Todo App</title>
    <style>
      :root {{ color-scheme: light dark; }}
      body {{
        font-family: system-ui, -apple-system, sans-serif;
        max-width: 720px; margin: 4rem auto; padding: 0 1rem;
        line-height: 1.5;
      }}
      h1 {{ margin-bottom: 0.25rem; }}
      .muted {{ color: #888; }}
      code {{ background: rgba(127,127,127,.15); padding: .1em .35em; border-radius: 4px; }}
      img.hourly {{
        max-width: 100%; height: auto; border-radius: 8px;
        margin: 1rem 0; display: block;
      }}
      form {{ display: flex; gap: 0.5rem; margin: 1rem 0; }}
      input[type="text"] {{
        flex: 1; padding: 0.5rem 0.75rem; font-size: 1rem;
        border: 1px solid rgba(127,127,127,.4); border-radius: 6px;
      }}
      button {{
        padding: 0.5rem 1.25rem; font-size: 1rem; cursor: pointer;
        border: 1px solid rgba(127,127,127,.4); border-radius: 6px;
        background: rgba(127,127,127,.15);
      }}
      .counter {{ font-size: 0.85rem; color: #888; margin-top: -0.5rem; }}
      ul {{ padding-left: 1.25rem; }}
      li {{ margin: 0.35rem 0; }}
      li input:disabled {{ accent-color: #888; }}
      li span.done {{ text-decoration: line-through; color: #888; }}
    </style>
  </head>
  <body>
    <h1>Todo App</h1>
    <p class="muted">DevOps with Kubernetes &mdash; Exercise 1.13</p>
    <p>This page is served by the <code>todo-app</code> pod running in the
       Kubernetes cluster, reached via an <code>Ingress</code> (Traefik).</p>

    <img class="hourly" src="/image" alt="Hourly picture from Lorem Picsum" />

    <h2>Add a todo</h2>
    <form id="todo-form" onsubmit="return false;">
      <input type="text" id="todo-input" maxlength="140"
             placeholder="New todo (max 140 characters)" autocomplete="off" />
      <button type="submit" id="send-btn">Send</button>
    </form>
    <p class="counter"><span id="char-count">0</span>/140 characters</p>

    <h2>Todos</h2>
    <ul id="todo-list">
      {todo_items}
    </ul>

    <h2>Available endpoints</h2>
    <ul>
      <li><code>GET /</code> &mdash; this page</li>
      <li><code>GET /image</code> &mdash; the hourly picture (cached in a PersistentVolume)</li>
      <li><code>GET /api/health</code> &mdash; <code>{{"status":"ok"}}</code></li>
      <li><code>GET /api/todos</code> &mdash; the hardcoded todos as JSON</li>
      <li><code>GET /shutdown</code> &mdash; exits the process (for testing persistence)</li>
    </ul>

    <script>
      const input = document.getElementById('todo-input');
      const count = document.getElementById('char-count');
      input.addEventListener('input', () => {{
        count.textContent = input.value.length;
      }});
      document.getElementById('send-btn').addEventListener('click', () => {{
        const value = input.value.trim();
        if (value.length === 0) {{
          alert('Please complete your answer first.');
          return;
        }}
        if (value.length > 140) {{
          alert('Todos must be 140 characters or less.');
          return;
        }}
        alert('Sending not implemented yet — coming in a later exercise!');
      }});
    </script>
  </body>
</html>"#
    ))
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
    println!("Shutting down on request (exercise 1.13 test)");
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

/// GET /api/todos — the hardcoded todo list as JSON (placeholder until
/// a later exercise adds real persistence).
async fn list_todos() -> Json<Vec<Todo>> {
    Json(hardcoded_todos())
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
