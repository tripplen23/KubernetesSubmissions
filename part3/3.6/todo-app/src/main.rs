use axum::{
    body::Body,
    extract::Form,
    http::{header, StatusCode},
    response::{Html, Redirect, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr};

#[derive(Serialize)]
struct Status {
    status: String,
}

/// A todo as returned by the todo-backend service.
#[derive(Serialize, Deserialize, Clone)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

/// The HTML form posts `content` (the todo text) to POST /todos.
#[derive(Deserialize)]
struct NewTodoForm {
    content: String,
}

/// Read a required env var, failing loudly if it is missing. No
/// hardcoded defaults — every value is injected by the Deployment or a
/// ConfigMap.
fn env_or(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}

/// GET / — server-side rendered page: hourly image + todo form +
/// the todo list fetched from todo-backend over HTTP.
async fn index() -> Html<String> {
    let backend_url = env_or("TODO_BACKEND_URL");

    // Fetch todos from the backend service (HTTP).
    let todos: Vec<Todo> = match reqwest::get(format!("{}/todos", backend_url)).await {
        Ok(resp) => resp.json().await.unwrap_or_default(),
        Err(e) => {
            eprintln!("Failed to fetch todos from {}: {}", backend_url, e);
            vec![]
        }
    };

    let todo_items: String = if todos.is_empty() {
        r#"<li class="muted">No todos yet — add one above!</li>"#.to_string()
    } else {
        todos
            .iter()
            .map(|t| {
                let checked = if t.done { " checked" } else { "" };
                let done_cls = if t.done { " class=\"done\"" } else { "" };
                format!(
                    r#"<li><label><input type="checkbox"{checked} disabled /> <span{done_cls}>{}</span></label></li>"#,
                    t.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n      ")
    };

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
      span.done {{ text-decoration: line-through; color: #888; }}
    </style>
  </head>
  <body>
    <h1>Todo App</h1>
    <p class="muted">DevOps with Kubernetes &mdash; Exercise 2.6</p>
    <p>This page is served by the <code>todo-app</code> pod; todos are
       stored by the <code>todo-backend</code> service (reached via its
       Service DNS name).</p>

    <img class="hourly" src="/image" alt="Hourly picture from Lorem Picsum" />

    <h2>Add a todo</h2>
    <form id="todo-form" method="post" action="/todos">
      <input type="text" id="todo-input" name="content" maxlength="140"
             placeholder="New todo (max 140 characters)" autocomplete="off" />
      <button type="submit" id="send-btn">Send</button>
    </form>
    <p class="counter"><span id="char-count">0</span>/140 characters</p>

    <h2>Todos</h2>
    <ul id="todo-list">
      {todo_items}
    </ul>

    <script>
      const input = document.getElementById('todo-input');
      const count = document.getElementById('char-count');
      input.addEventListener('input', () => {{
        count.textContent = input.value.length;
      }});
    </script>
  </body>
</html>"#
    ))
}

/// POST /todos — receives the HTML form, forwards the todo to the
/// todo-backend service, then redirects back to the page.
async fn create_todo(Form(form): Form<NewTodoForm>) -> Result<Redirect, StatusCode> {
    let backend_url = env_or("TODO_BACKEND_URL");
    let title = form.content.trim().to_string();
    if title.is_empty() || title.chars().count() > 140 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let client = reqwest::Client::new();
    match client
        .post(format!("{}/todos", backend_url))
        .json(&serde_json::json!({ "title": title }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => Ok(Redirect::to("/")),
        Ok(resp) => {
            eprintln!("todo-backend rejected: {}", resp.status());
            Err(StatusCode::BAD_GATEWAY)
        }
        Err(e) => {
            eprintln!("Failed to reach todo-backend: {}", e);
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

/// GET /image — serves the cached picture if it is younger than the
/// configured max age; otherwise fetches a fresh one from the
/// configured image URL, stores it on the PersistentVolume and serves it.
async fn image() -> Response {
    let path = env_or("IMAGE_PATH");
    let image_url = env_or("IMAGE_URL");
    let max_age: u64 = env_or("MAX_AGE_SECS")
        .parse()
        .expect("MAX_AGE_SECS must be a valid number");

    if let Ok(meta) = tokio::fs::metadata(&path).await {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = modified.elapsed() {
                if age.as_secs() < max_age {
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

    match reqwest::get(&image_url).await {
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

#[tokio::main]
async fn main() {
    let port: u16 = env_or("PORT")
        .parse()
        .expect("PORT must be a valid number");

    let app = Router::new()
        .route("/", get(index))
        .route("/todos", axum::routing::post(create_todo))
        .route("/image", get(image))
        .route("/api/health", get(health));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
