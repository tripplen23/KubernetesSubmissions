//! dummysite-server: serves a copy of one URL as an HTML page.
//!
//! The DummySite controller creates this container with WEBSITE_URL set from the
//! DummySite object. The page is fetched once at startup, kept in memory, and refreshed
//! on a delay so the copy does not go stale.

use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, routing::get, Router};
use tokio::sync::RwLock;

const NOT_FETCHED_YET: &str = "<h1>the site has not been fetched yet</h1>";

#[derive(Clone)]
struct AppState {
    website_url: String,
    page: Arc<RwLock<String>>,
}

/// Fail fast: a missing environment variable should crash loudly, not serve a wrong page.
fn env_or(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("environment variable {name} is not set"))
}

async fn fetch_page(url: &str) -> Option<String> {
    let response = reqwest::get(url).await.ok()?;
    response.text().await.ok()
}

async fn page(State(state): State<AppState>) -> String {
    state.page.read().await.clone()
}

async fn healthz() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    let website_url = env_or("WEBSITE_URL");
    let state = AppState {
        website_url: website_url.clone(),
        page: Arc::new(RwLock::new(NOT_FETCHED_YET.to_string())),
    };

    tokio::spawn({
        let state = state.clone();
        async move {
            loop {
                match fetch_page(&state.website_url).await {
                    // Keep the previous copy if a fetch fails: a temporarily unreachable
                    // site should not empty the page.
                    Some(page) => {
                        println!("fetched {} -> {} characters", state.website_url, page.len());
                        *state.page.write().await = page;
                    }
                    None => eprintln!("fetching {} failed", state.website_url),
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    });

    let app = Router::new()
        .route("/", get(page))
        .route("/healthz", get(healthz))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("cannot bind port 3000");
    println!("dummysite-server for {website_url} listening on port 3000");
    axum::serve(listener, app).await.expect("server failed");
}