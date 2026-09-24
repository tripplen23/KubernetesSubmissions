use axum::{routing::get, Router};
use std::env;
use std::net::SocketAddr;

/// The greeting this build answers with when GREETING is not set.
const DEFAULT_GREETING: &str = "hello";

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    // One image, two versions: which greeting a Deployment answers with is
    // decided by its own environment, not by the image tag.
    let greeting = env::var("GREETING").unwrap_or_else(|_| DEFAULT_GREETING.to_string());

    println!("Server started in port {} greeting: {}", port, greeting);

    let app = Router::new().route("/", get(move || greet(greeting.clone())));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET / → the greeting, as plain text.
async fn greet(greeting: String) -> String {
    greeting
}