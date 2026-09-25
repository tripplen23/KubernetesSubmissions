use axum::{routing::get, Router};
use std::env;
use std::net::SocketAddr;

/// The Knative runtime contract in one file: a serverless app is stateless,
/// reads its configuration from the environment, listens on the port the
/// platform gives it, logs to stdout, and leaves when the platform asks.
const DEFAULT_PORT: &str = "8080";
const DEFAULT_TARGET: &str = "World";

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| DEFAULT_PORT.to_string())
        .parse()
        .expect("PORT must be a valid number");

    // One image, many revisions: which answer a revision gives is decided by
    // its own environment, exactly like the greeter in 5.3.
    let target = env::var("TARGET").unwrap_or_else(|_| DEFAULT_TARGET.to_string());

    // Knative sets K_REVISION for every revision it runs; outside Knative the
    // variable is simply absent, which is also worth saying out loud.
    let revision = env::var("K_REVISION").unwrap_or_else(|_| "outside-knative".to_string());

    println!("listening on {port}, TARGET={target}, K_REVISION={revision}");

    let app = Router::new()
        .route("/", get(move || hello(target.clone(), revision.clone())))
        .route("/work", get(work));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

/// GET / → the greeting and the revision that answered it, as plain text.
async fn hello(target: String, revision: String) -> String {
    format!("Hello {target}! (answered by {revision})\n")
}

/// GET /work?ms=500 → answers after that long. This is the autoscaling demo's
/// knob: Knative scales on requests that are *in flight*, and a request that
/// returns in milliseconds never gives the autoscaler anything to see.
async fn work(uri: axum::http::Uri) -> String {
    let ms = uri
        .query()
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("ms=")))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(500)
        .min(10_000);
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    format!("slept {ms}ms\n")
}

/// The contract's "should": drain on SIGTERM instead of dropping the request
/// the autoscaler is still routing here while it scales the pod down.
async fn shutdown_signal() {
    let mut term =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
    println!("shutting down");
}
