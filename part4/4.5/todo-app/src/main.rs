use axum::{
    body::Body,
    extract::{Form, Path, State},
    http::{header, StatusCode},
    response::{Html, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// The "break" switch from exercise 4.2 (the button on the front page) plus
/// everything else the handlers share. The flag lives in the process memory and
/// nowhere else — that is the whole point: the liveness probe restarts the
/// container and the app comes back healthy, because the flag dies with the
/// process.
#[derive(Clone)]
struct AppState {
    broken: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct Status {
    status: String,
    /// Only present when the check failed; handy in `kubectl describe pod`.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Status {
    fn ok() -> Self {
        Status {
            status: "ok".to_string(),
            error: None,
        }
    }

    fn unhealthy(error: impl Into<String>) -> Self {
        Status {
            status: "unhealthy".to_string(),
            error: Some(error.into()),
        }
    }
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

/// The "Done"/"Undo" button posts the value it wants the todo to have, so the
/// app does not have to read the todo first to flip it.
#[derive(Deserialize)]
struct DoneForm {
    #[serde(default)]
    done: Option<String>,
}

/// Read a required env var, failing loudly if it is missing. No
/// hardcoded defaults — every value is injected by the Deployment or a
/// ConfigMap.
fn env_or(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}

/// Optional env var with a default. `VERSION` is the one value a Deployment may
/// leave out: it only labels the page (and, in this exercise, it is how you see
/// which of two versions the BlueGreen preview is serving).
fn env_or_default(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Todo titles are user input: escape the three characters that would otherwise
/// turn a todo into markup (`<script>`, `&amp;`, a stray quote).
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// GET / — server-side rendered page: hourly image + todo form + the todo list
/// fetched from todo-backend over HTTP, with the buttons of exercise 4.5.
async fn index(State(state): State<AppState>) -> Html<String> {
    let backend_url = env_or("TODO_BACKEND_URL");
    let version = env_or_default("VERSION", "v1");

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
                // The button offers the other state, and says which one it is.
                let (label, next) = if t.done { ("Undo", "false") } else { ("Done", "true") };
                format!(
                    r#"<li class="todo">
        <label><input type="checkbox"{checked} disabled /> <span{done_cls}>{title}</span></label>
        <form method="post" action="/todos/{id}/done">
          <input type="hidden" name="done" value="{next}" />
          <button type="submit" class="small">{label}</button>
        </form>
        <form method="post" action="/todos/{id}/delete">
          <button type="submit" class="small danger">Delete</button>
        </form>
      </li>"#,
                    id = t.id,
                    title = escape(&t.title),
                )
            })
            .collect::<Vec<_>>()
            .join("\n      ")
    };

    let done_count = todos.iter().filter(|t| t.done).count();

    // Exercise 4.2: the page shows whether the app has been broken from the UI.
    let broken_banner = if state.broken.load(Ordering::Relaxed) {
        r#"<p class="broken">⚠️ This instance has been <strong>broken</strong> from the UI.
       Its <code>/healthz</code> and <code>/livez</code> answer 500, so Kubernetes has
       taken it out of the Service — and the liveness probe is about to restart the
       container, which brings the app back healthy.</p>"#
    } else {
        ""
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
      button.small {{ padding: 0.25rem 0.7rem; font-size: 0.85rem; }}
      button.danger {{ border-color: #b00; color: #b00; }}
      .counter {{ font-size: 0.85rem; color: #888; margin-top: -0.5rem; }}
      ul {{ padding-left: 0; list-style: none; }}
      li.todo {{
        display: flex; align-items: center; gap: 0.5rem;
        padding: 0.35rem 0; border-bottom: 1px solid rgba(127,127,127,.15);
      }}
      li.todo label {{ flex: 1; display: flex; align-items: center; gap: 0.5rem; }}
      li.todo form {{ margin: 0; }}
      li input:disabled {{ accent-color: #888; }}
      span.done {{ text-decoration: line-through; color: #888; }}
      .broken {{
        border: 1px solid #b00; border-left: 4px solid #b00; border-radius: 6px;
        background: rgba(187,0,0,.08); padding: 0.75rem 1rem;
      }}
      button.break {{ border-color: #b00; color: #b00; }}
    </style>
  </head>
  <body>
    <h1>Todo App</h1>
    <p class="muted">version <code>{version}</code> &mdash; DevOps with Kubernetes</p>
    <p>This page is served by the <code>todo-app</code> pod; todos are
       stored by the <code>todo-backend</code> service (reached via its
       Service DNS name).</p>

    {broken_banner}

    <img class="hourly" src="/image" alt="Hourly picture from Lorem Picsum" />

    <h2>Add a todo</h2>
    <form id="todo-form" method="post" action="/todos">
      <input type="text" id="todo-input" name="content" maxlength="140"
             placeholder="New todo (max 140 characters)" autocomplete="off" />
      <button type="submit" id="send-btn">Send</button>
    </form>
    <p class="counter"><span id="char-count">0</span>/140 characters</p>

    <h2>Todos <span class="muted">({done_count} done)</span></h2>
    <ul id="todo-list">
      {todo_items}
    </ul>

    <h2>Break it (exercise 4.2)</h2>
    <p class="muted">Pressing this makes <em>this</em> instance stop working: the
       health endpoints start answering 500 and the pod is taken out of the
       Service. Kubernetes then restarts the container and the app is healthy
       again by itself.</p>
    <form method="post" action="/break">
      <button type="submit" class="break" id="break-btn">Break the app</button>
    </form>

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

/// POST /break — the button on the front page. Flips the in-memory switch and
/// goes back to the page, which now shows the "broken" banner.
async fn break_app(State(state): State<AppState>) -> Redirect {
    state.broken.store(true, Ordering::Relaxed);
    println!("[break] the app was broken from the UI: /healthz and /livez now answer 500");
    Redirect::to("/")
}

/// GET /healthz — the endpoint the **readiness** probe calls.
///
/// It answers 200 only when both things are true: this instance has not been
/// broken from the UI, *and* the todo-backend (which is the component that
/// talks to Postgres) answers its own `/healthz`. That is the exercise's "the
/// app is working and connected to a database", seen from the app's side.
///
/// This is a readiness check and not a liveness check on purpose: a probe that
/// restarts a container because a *dependency* is down would turn a database
/// restart into a crash loop.
async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<Status>) {
    if state.broken.load(Ordering::Relaxed) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Status::unhealthy("broken from the UI")),
        );
    }

    let backend_url = env_or("TODO_BACKEND_URL");
    match reqwest::get(format!("{backend_url}/healthz")).await {
        Ok(resp) if resp.status().is_success() => (StatusCode::OK, Json(Status::ok())),
        Ok(resp) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Status::unhealthy(format!(
                "todo-backend answered {}",
                resp.status()
            ))),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Status::unhealthy(format!("todo-backend unreachable: {e}"))),
        ),
    }
}

/// GET /livez — the endpoint the **liveness** probe calls.
///
/// Only the process's own state matters here: the app is alive unless it has
/// been deliberately broken from the UI. No dependency is consulted, so a
/// database that is restarting never causes a restart of the app.
async fn livez(State(state): State<AppState>) -> (StatusCode, Json<Status>) {
    if state.broken.load(Ordering::Relaxed) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Status::unhealthy("broken from the UI")),
        )
    } else {
        (StatusCode::OK, Json(Status::ok()))
    }
}

/// POST /todos — receives the HTML form, forwards the todo to the
/// todo-backend service, then redirects back to the page.
async fn create_todo(
    State(state): State<AppState>,
    Form(form): Form<NewTodoForm>,
) -> Result<Redirect, StatusCode> {
    // Exercise 4.2: while the app is "broken" its normal operation stops.
    if state.broken.load(Ordering::Relaxed) {
        println!("[break] refusing to create a todo: this instance is broken");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

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

/// POST /todos/<id>/done — the "Done"/"Undo" button (exercise 4.5).
///
/// An HTML form can only GET and POST, so this handler is the adapter: it
/// forwards the change to the backend as the `PUT /todos/<id>` the exercise
/// asks for, then sends the browser back to the page.
async fn toggle_done(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Form(form): Form<DoneForm>,
) -> Result<Redirect, StatusCode> {
    if state.broken.load(Ordering::Relaxed) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    // The form carries the state the todo should end up in.
    let done = form.done.as_deref() == Some("true");
    let backend_url = env_or("TODO_BACKEND_URL");

    let response = reqwest::Client::new()
        .put(format!("{backend_url}/todos/{id}"))
        .json(&serde_json::json!({ "done": done }))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            println!("[4.5] todo {id} -> done={done}");
            Ok(Redirect::to("/"))
        }
        Ok(resp) => {
            eprintln!("todo-backend rejected PUT /todos/{id}: {}", resp.status());
            Err(StatusCode::BAD_GATEWAY)
        }
        Err(e) => {
            eprintln!("Failed to reach todo-backend: {}", e);
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

/// POST /todos/<id>/delete — the "Delete" button. Same story: the browser POSTs
/// a form, the backend gets the `DELETE /todos/<id>` it deserves.
async fn delete_todo(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Redirect, StatusCode> {
    if state.broken.load(Ordering::Relaxed) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let backend_url = env_or("TODO_BACKEND_URL");
    let response = reqwest::Client::new()
        .delete(format!("{backend_url}/todos/{id}"))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND => {
            println!("[4.5] todo {id} deleted ({})", resp.status());
            Ok(Redirect::to("/"))
        }
        Ok(resp) => {
            eprintln!("todo-backend rejected DELETE /todos/{id}: {}", resp.status());
            Err(StatusCode::BAD_GATEWAY)
        }
        Err(e) => {
            eprintln!("Failed to reach todo-backend: {}", e);
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

/// GET /image — serves the cached picture if it is younger than the configured
/// max age; otherwise fetches a fresh one from the configured image URL, stores
/// it on the PersistentVolume and serves it.
///
/// Two things this handler learned the hard way, both of them about running
/// inside a *private* cluster:
///
/// * the image source has to be reachable from the nodes — the default
///   `picsum.photos` is not, and the page then shows a broken image;
/// * a fetch that fails is not a reason to show nothing: if there is a picture
///   on the volume, even an expired one, it is served. An old picture beats a
///   broken one.
async fn image() -> Response {
    let path = env_or("IMAGE_PATH");
    let image_url = env_or("IMAGE_URL");
    let max_age: u64 = env_or("MAX_AGE_SECS")
        .parse()
        .expect("MAX_AGE_SECS must be a valid number");

    // Whatever is on the volume right now, plus how old it is.
    let cached = match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let age = tokio::fs::metadata(&path)
                .await
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|mtime| mtime.elapsed().ok())
                .map(|age| age.as_secs());
            Some((bytes, age))
        }
        Err(_) => None,
    };

    // Young enough? Then the image service is not asked at all.
    if let Some((bytes, Some(age))) = &cached {
        if *age < max_age {
            println!("Serving cached image (age {}s)", age);
            return image_response(bytes.clone());
        }
    }

    match reqwest::get(&image_url).await {
        Ok(resp) if resp.status().is_success() => match resp.bytes().await {
            Ok(bytes) => {
                let bytes = bytes.to_vec();
                match tokio::fs::write(&path, &bytes).await {
                    Ok(_) => println!("Cached new image to {}", path),
                    Err(e) => eprintln!("Failed to cache image: {}", e),
                }
                image_response(bytes)
            }
            Err(e) => {
                eprintln!("Failed to read image body: {}", e);
                stale_or_error(cached)
            }
        },
        Ok(resp) => {
            eprintln!("Image source answered {}", resp.status());
            stale_or_error(cached)
        }
        Err(e) => {
            eprintln!("Failed to fetch image: {}", e);
            stale_or_error(cached)
        }
    }
}

/// A picture from the volume, even an expired one, is still a picture.
fn stale_or_error(cached: Option<(Vec<u8>, Option<u64>)>) -> Response {
    match cached {
        Some((bytes, _)) => {
            println!("Serving the stale cached image instead of failing");
            image_response(bytes)
        }
        None => error_response(),
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
    Json(Status::ok())
}

#[tokio::main]
async fn main() {
    let port: u16 = env_or("PORT")
        .parse()
        .expect("PORT must be a valid number");

    let app = Router::new()
        .route("/", get(index))
        .route("/todos", post(create_todo))
        // Exercise 4.5, seen from the browser: both buttons speak form-POST to
        // this app, which speaks PUT/DELETE to the backend.
        .route("/todos/{id}/done", post(toggle_done))
        .route("/todos/{id}/delete", post(delete_todo))
        .route("/image", get(image))
        .route("/api/health", get(health))
        .route("/healthz", get(healthz))
        .route("/livez", get(livez))
        .route("/break", post(break_app))
        .with_state(AppState {
            broken: Arc::new(AtomicBool::new(false)),
        });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}