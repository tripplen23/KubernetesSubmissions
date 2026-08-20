use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use tokio_postgres::{Client, NoTls};

#[derive(Serialize, Clone)]
struct Todo {
    id: i64,
    title: String,
    done: bool,
}

/// Body of POST /todos — the only required field is `title`.
#[derive(Deserialize)]
struct NewTodo {
    title: String,
}

/// Shared Postgres connection config. The todos live in a Postgres
/// StatefulSet, and the app opens a fresh connection per request
/// so it auto-recovers when the database pod restarts.
#[derive(Clone)]
struct AppState {
    config: tokio_postgres::Config,
}

/// Read a required env var, failing loudly if it is missing. Every value
/// comes from a ConfigMap or a Secret.
fn env_or(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
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

    let app = Router::new()
        .route("/todos", get(list_todos).post(create_todo))
        .with_state(AppState { config });

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
async fn create_todo(
    State(state): State<AppState>,
    Json(new_todo): Json<NewTodo>,
) -> Result<(StatusCode, Json<Todo>), StatusCode> {
    let title = new_todo.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 140 {
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
            Ok((StatusCode::CREATED, Json(todo)))
        }
        Err(e) => {
            eprintln!("Failed to insert todo: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
