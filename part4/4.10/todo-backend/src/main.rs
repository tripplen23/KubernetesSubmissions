use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio_postgres::{Client, NoTls};

#[derive(Serialize, Clone)]
struct Todo {
    id: i64,
    title: String,
    done: bool,
}

/// Body of `GET /healthz` — what the readiness probe reads.
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

    fn unhealthy(error: String) -> Self {
        Status {
            status: "unhealthy".to_string(),
            error: Some(error),
        }
    }
}

/// Body of POST /todos — the only required field is `title`.
#[derive(Deserialize)]
struct NewTodo {
    title: String,
}

/// Body of `PUT /todos/<id>` — exercise 4.5. The todo is "done" or it is not;
/// the client says which, the backend does not guess.
#[derive(Deserialize)]
struct UpdateTodo {
    done: bool,
}

/// Body of `GET /version` — this lab's BlueGreen demo needs a way to see *which*
/// version answered, and a JSON field is friendlier than reading pod names.
#[derive(Serialize)]
struct Version {
    version: String,
}

/// Shared state: the Postgres connection config, plus — when `NATS_URL` is set —
/// the client the todo events are published with.
#[derive(Clone)]
struct AppState {
    config: tokio_postgres::Config,
    /// `None` when `NATS_URL` is not set. The API works without NATS; it just
    /// does not announce anything.
    nats: Option<async_nats::Client>,
    subject: String,
}

/// Read a required env var, failing loudly if it is missing. Every value
/// comes from a ConfigMap or a Secret.
fn env_or(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}

/// Optional env var with a default — only `VERSION` uses this: it labels which
/// release is answering, and a missing label must not keep the app from starting.
fn env_or_default(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Connect to NATS, retrying while it starts. Returns `None` when `NATS_URL` is
/// not set at all: exercise 4.6 adds messaging to an API that has to keep working
/// without it.
async fn connect_nats() -> (Option<async_nats::Client>, String) {
    let subject = env_or_default("NATS_SUBJECT", "todo_events");
    let url = match env::var("NATS_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("NATS_URL is not set — todo events will not be published");
            return (None, subject);
        }
    };

    let mut attempt = 0u32;
    loop {
        match async_nats::connect(&url).await {
            Ok(client) => {
                println!("publishing todo events to {subject} on {url}");
                return (Some(client), subject);
            }
            Err(e) => {
                attempt += 1;
                if attempt >= 30 {
                    eprintln!("NATS unreachable after {attempt} attempts ({e}) — continuing without it");
                    return (None, subject);
                }
                eprintln!("NATS not ready ({e}) — retrying in 2s ({attempt}/30)…");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Announce one todo event. This never fails the request: the todo is saved in
/// Postgres either way, and a message that could not be published must not turn
/// into a write the user sees as failed.
async fn publish_event(state: &AppState, event: &str, todo: &Todo) {
    let Some(nats) = &state.nats else { return };
    let payload = serde_json::json!({
        "event": event,
        "todo": { "id": todo.id, "title": todo.title, "done": todo.done },
    });
    match nats
        .publish(state.subject.clone(), payload.to_string().into())
        .await
    {
        Ok(_) => println!("[nats] {event} -> {} {payload}", state.subject),
        Err(e) => eprintln!("[nats] publish failed: {e}"),
    }
}

/// Request logger middleware — prints one line to stdout for every
/// request that hits the backend. These lines are what Alloy scrapes
/// from the node filesystem and forwards to Loki, so they show up in
/// Grafana. Every request (including blocked 400s) is logged.
async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let res = next.run(req).await;

    let status = res.status();
    let ms = start.elapsed().as_millis();
    println!("[req] {method} {uri} -> {status} ({ms} ms)");
    res
}

#[tokio::main]
async fn main() {
    let port: u16 = env_or("PORT").parse().expect("PORT must be a valid number");

    // Build the Postgres connection config from the pieces injected by a
    // ConfigMap (non-secret) + a Secret (the password). We assemble the
    // URL in code so the password never sits in a plain-text manifest.
    let user = env_or("POSTGRES_USER");
    let password = env_or("POSTGRES_PASSWORD");
    let host = env_or("POSTGRES_HOST");
    let db_port = env_or("POSTGRES_PORT");
    let dbname = env_or("POSTGRES_DB");
    let database_url = format!("postgres://{user}:{password}@{host}:{db_port}/{dbname}");
    let config: tokio_postgres::Config = database_url
        .parse()
        .expect("built DATABASE_URL must be a valid postgres URL");

    // Create the table once at startup, retrying while Postgres boots.
    init_db(&config).await;

    // Exercise 4.6: every saved or updated todo is announced on NATS. The
    // connection is optional on purpose — see `connect_nats`.
    let (nats, subject) = connect_nats().await;

    let app = Router::new()
        .route("/todos", get(list_todos).post(create_todo))
        // Exercise 4.5: a todo is updated (marked done, or undone) with a PUT,
        // and removed with a DELETE. Same path, two verbs, one resource.
        .route("/todos/{id}", axum::routing::put(update_todo).delete(delete_todo))
        .route("/version", get(version))
        .route("/healthz", get(healthz))
        .layer(middleware::from_fn(log_request))
        .with_state(AppState {
            config,
            nats,
            subject,
        });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("todo-backend started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn init_db(config: &tokio_postgres::Config) {
    let mut attempts = 0u32;
    loop {
        match config.connect(NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("Postgres connection error: {}", e);
                    }
                });
                client
                    .execute(
                        "CREATE TABLE IF NOT EXISTS todos (id BIGSERIAL PRIMARY KEY, title TEXT NOT NULL, done BOOLEAN NOT NULL DEFAULT false)",
                        &[],
                    )
                    .await
                    .expect("create todos table");
                // Exercise 4.5 arrives on top of a database that already has
                // todos in it, so the new column is added with ALTER TABLE —
                // it is a no-op when the column is already there.
                client
                    .execute(
                        "ALTER TABLE todos ADD COLUMN IF NOT EXISTS done BOOLEAN NOT NULL DEFAULT false",
                        &[],
                    )
                    .await
                    .expect("add done column");
                return;
            }
            Err(e) => {
                attempts += 1;
                if attempts >= 30 {
                    panic!("Postgres not reachable after {attempts} attempts: {e}");
                }
                eprintln!("Postgres not ready ({e}) — retrying in 2s ({attempts}/30)…");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Open a fresh connection per request (auto-recovery on DB restart).
async fn connect(config: &tokio_postgres::Config) -> Result<Client, String> {
    let (client, connection) = config.connect(NoTls).await.map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

/// `GET /healthz` — the endpoint the **readiness** probe calls (exercise 4.2:
/// "ensure that it's working and connected to a database").
///
/// Nothing is cached and nothing is assumed: on every check the backend opens a
/// fresh connection (the same way the request handlers do) and runs `SELECT 1`,
/// so a Postgres that goes away — or comes back — is noticed within one probe
/// period. While this answers 500 the pod is *not Ready*: it keeps running, but
/// the Service takes it out of its endpoints, so the todo app stops sending it
/// traffic.
async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<Status>) {
    let client = match connect(&state.config).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("healthz: cannot connect to Postgres: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Status::unhealthy(e)),
            );
        }
    };

    match client.query_one("SELECT 1", &[]).await {
        Ok(_) => (StatusCode::OK, Json(Status::ok())),
        Err(e) => {
            eprintln!("healthz: query failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Status::unhealthy(e.to_string())),
            )
        }
    }
}

/// GET /todos → the full list as JSON, ordered by id.
async fn list_todos(State(state): State<AppState>) -> Json<Vec<Todo>> {
    let client = match connect(&state.config).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to Postgres: {}", e);
            return Json(vec![]);
        }
    };
    match client
        .query("SELECT id, title, done FROM todos ORDER BY id", &[])
        .await
    {
        Ok(rows) => {
            let todos = rows
                .iter()
                .map(|r| Todo {
                    id: r.get(0),
                    title: r.get(1),
                    done: r.get(2),
                })
                .collect();
            Json(todos)
        }
        Err(e) => {
            eprintln!("Failed to list todos: {}", e);
            Json(vec![])
        }
    }
}

/// POST /todos — body: {"title": "..."} → creates a todo (id from the
/// DB sequence), replies 201 with the created todo.
///
/// Exercise 2.10: the limit of 140 characters for a todo is enforced here
/// in the backend too (the UI already limited it). A todo longer than 140
/// characters (or empty) is rejected with `400 Bad Request`. The rejection
/// is logged (see `log_request` middleware), so "non-allowed" messages are
/// visible in Grafana.
async fn create_todo(
    State(state): State<AppState>,
    Json(new_todo): Json<NewTodo>,
) -> Result<(StatusCode, Json<Todo>), StatusCode> {
    let title = new_todo.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 140 {
        // Rejected — the middleware still logs 400, so the blocked todo
        // shows up in Grafana (exercise 2.10 requirement).
        eprintln!("[reject] todo blocked: {} chars (max 140)", title.chars().count());
        return Err(StatusCode::BAD_REQUEST);
    }

    let client = connect(&state.config)
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to Postgres: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match client
        .query_one(
            "INSERT INTO todos (title) VALUES ($1) RETURNING id, title, done",
            &[&title],
        )
        .await
    {
        Ok(row) => {
            let todo = Todo {
                id: row.get(0),
                title: row.get(1),
                done: row.get(2),
            };
            publish_event(&state, "created", &todo).await;
            Ok((StatusCode::CREATED, Json(todo)))
        }
        Err(e) => {
            eprintln!("Failed to insert todo: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// `GET /version` — which release is answering. Used by this lab's BlueGreen
/// step: the preview Service and the active Service point at *different*
/// ReplicaSets, and this is how you tell them apart from a shell.
async fn version() -> Json<Version> {
    Json(Version {
        version: env_or_default("VERSION", "v1"),
    })
}

/// `PUT /todos/<id>` — exercise 4.5: *"Our todo application could use a Done
/// field for todos that are already done. It should be a PUT request to
/// `/todos/<id>`."*
///
/// The SQL is one statement, and `RETURNING` gives the updated row back, so the
/// response is the todo as it now is. An id that does not exist is a `404`, not
/// a silent success.
async fn update_todo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateTodo>,
) -> Result<Json<Todo>, StatusCode> {
    let client = connect(&state.config)
        .await
        .map_err(|e| {
            eprintln!("Failed to connect to Postgres: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match client
        .query_opt(
            "UPDATE todos SET done = $1 WHERE id = $2 RETURNING id, title, done",
            &[&update.done, &id],
        )
        .await
    {
        Ok(Some(row)) => {
            let todo = Todo {
                id: row.get(0),
                title: row.get(1),
                done: row.get(2),
            };
            // "saving or updating todos" — an update is an event too.
            publish_event(&state, if todo.done { "done" } else { "undone" }, &todo).await;
            Ok(Json(todo))
        }
        Ok(None) => {
            println!("[404] no todo with id {id}");
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            eprintln!("Failed to update todo {id}: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// `DELETE /todos/<id>` — the delete button of this lab's UI.
///
/// `204 No Content` when a row was removed, `404` when the id was not there, so
/// the caller can tell "deleted" from "never existed" without fetching the list
/// again.
async fn delete_todo(State(state): State<AppState>, Path(id): Path<i64>) -> StatusCode {
    let client = match connect(&state.config).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to Postgres: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    match client
        .query_opt(
            "DELETE FROM todos WHERE id = $1 RETURNING id, title, done",
            &[&id],
        )
        .await
    {
        Ok(Some(row)) => {
            let todo = Todo {
                id: row.get(0),
                title: row.get(1),
                done: row.get(2),
            };
            publish_event(&state, "deleted", &todo).await;
            StatusCode::NO_CONTENT
        }
        Ok(None) => {
            println!("[404] no todo with id {id} to delete");
            StatusCode::NOT_FOUND
        }
        Err(e) => {
            eprintln!("Failed to delete todo {id}: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}