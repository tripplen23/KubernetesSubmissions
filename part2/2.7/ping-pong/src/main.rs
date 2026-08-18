use axum::{extract::State, routing::get, Router};
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use tokio_postgres::{Client, NoTls};

/// Shared Postgres connection config. We open a fresh connection per
/// request (see 2.7) so the app recovers automatically when the
/// Postgres pod restarts — no stale connection to reconnect by hand.
#[derive(Clone)]
struct AppState {
    config: tokio_postgres::Config,
}

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");
    let database_url = env::var("DATABASE_URL")
        .expect("environment variable DATABASE_URL is not set");

    let config: tokio_postgres::Config = database_url
        .parse()
        .expect("DATABASE_URL must be a valid postgres URL");

    // Create the table and seed the single counter row once at startup,
    // retrying while the StatefulSet is still booting.
    init_db(&config).await;

    let app = Router::new()
        .route("/pingpong", get(pong))
        .route("/pongs", get(pongs))
        .with_state(AppState { config });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started in port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Connect + ensure the schema exists, retrying until the database is
/// reachable (the Postgres StatefulSet may still be starting).
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
                        "CREATE TABLE IF NOT EXISTS pongs (id SERIAL PRIMARY KEY, count BIGINT NOT NULL DEFAULT 0)",
                        &[],
                    )
                    .await
                    .expect("create pongs table");
                client
                    .execute(
                        "INSERT INTO pongs (id, count) VALUES (1, 0) ON CONFLICT (id) DO NOTHING",
                        &[],
                    )
                    .await
                    .expect("seed pongs row");
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

/// Open a fresh connection for a single request. Each request gets its
/// own connection, so if Postgres restarted the next request simply
/// opens a new one and succeeds.
async fn connect(config: &tokio_postgres::Config) -> Result<Client, String> {
    let (client, connection) = config.connect(NoTls).await.map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

/// GET /pingpong → "pong 0", "pong 1", ...
///
/// Atomically increments the counter in Postgres and replies with the
/// previous value (same behaviour as before, but persisted).
async fn pong(State(state): State<AppState>) -> String {
    let client = match connect(&state.config).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to Postgres: {}", e);
            return "database error".to_string();
        }
    };
    match client
        .query_one(
            "UPDATE pongs SET count = count + 1 WHERE id = 1 RETURNING count",
            &[],
        )
        .await
    {
        Ok(r) => {
            let new_count: i64 = r.get(0);
            format!("pong {}", new_count - 1)
        }
        Err(e) => {
            eprintln!("Failed to increment pong counter: {}", e);
            "database error".to_string()
        }
    }
}

/// GET /pongs → "3" (the current number of pongs, no increment)
async fn pongs(State(state): State<AppState>) -> String {
    let client = match connect(&state.config).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to Postgres: {}", e);
            return "database error".to_string();
        }
    };
    match client
        .query_one("SELECT count FROM pongs WHERE id = 1", &[])
        .await
    {
        Ok(r) => {
            let count: i64 = r.get(0);
            count.to_string()
        }
        Err(e) => {
            eprintln!("Failed to read pong counter: {}", e);
            "database error".to_string()
        }
    }
}
