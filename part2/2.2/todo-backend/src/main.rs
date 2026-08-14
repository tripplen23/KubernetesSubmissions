use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

/// Body of POST /todos — the only required field is `title`.
#[derive(Deserialize)]
struct NewTodo {
    title: String,
}

/// In-memory storage (a database comes in a later exercise).
#[derive(Default)]
struct TodoStore {
    todos: Vec<Todo>,
    next_id: u64,
}

type SharedStore = Arc<Mutex<TodoStore>>;

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let store: SharedStore = Arc::new(Mutex::new(TodoStore::default()));

    let app = Router::new()
        .route("/todos", get(list_todos).post(create_todo))
        .with_state(store);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("todo-backend started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET /todos → the full list as JSON.
async fn list_todos(State(store): State<SharedStore>) -> Json<Vec<Todo>> {
    let store = store.lock().unwrap();
    Json(store.todos.clone())
}

/// POST /todos — body: {"title": "..."} → creates a todo (id auto-assigned),
/// replies 201 with the created todo.
async fn create_todo(
    State(store): State<SharedStore>,
    Json(new_todo): Json<NewTodo>,
) -> Result<(StatusCode, Json<Todo>), StatusCode> {
    let title = new_todo.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 140 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut store = store.lock().unwrap();
    let todo = Todo {
        id: store.next_id,
        title,
        done: false,
    };
    store.next_id += 1;
    store.todos.push(todo.clone());
    Ok((StatusCode::CREATED, Json(todo)))
}
