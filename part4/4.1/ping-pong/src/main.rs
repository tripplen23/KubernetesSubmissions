use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Router,
};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

/// ping-pong — counts "pongs" in Postgres.
///
///   GET /        → increments the counter in the database, answers "pong N"
///   GET /healthz → 200 only while the database answers; 500 when it does not
///                  (this is what the readinessProbe in the lab points at)
///
/// Configuration comes from the environment (see the manifests):
/// POSTGRES_HOST, POSTGRES_PORT, POSTGRES_USER, POSTGRES_PASSWORD, POSTGRES_DB.
///
/// The database connection is maintained in the background, so the process
/// starts even when Postgres is not up yet — it is simply NOT READY until the
/// connection succeeds. That is the whole point of the exercise.
#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Option<Client>>>,
}

/// Every database call is capped, so a probe never hangs for seconds when the
/// database has gone away.
const QUERY_TIMEOUT: Duration = Duration::from_millis(800);
const RETRY_DELAY: Duration = Duration::from_secs(2);

struct ConnectionConfig {
    host: String,
    port: u16,
    user: String,
    password: String,
    database: String,
}

#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3541".to_string())
        .parse()
        .expect("PORT must be a valid number");

    let config = ConnectionConfig {
        host: env::var("POSTGRES_HOST").unwrap_or_else(|_| "postgres-svc".to_string()),
        port: env::var("POSTGRES_PORT")
            .unwrap_or_else(|_| "5432".to_string())
            .parse()
            .expect("POSTGRES_PORT must be a valid number"),
        user: env::var("POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string()),
        password: env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "postgres".to_string()),
        database: env::var("POSTGRES_DB").unwrap_or_else(|_| "postgres".to_string()),
    };

    let state = AppState {
        db: Arc::new(Mutex::new(None)),
    };
    tokio::spawn(maintain_connection(state.clone(), config));

    let app = Router::new()
        .route("/", get(root))
        .route("/healthz", get(healthz))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("ping-pong listening on port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn connect(config: &ConnectionConfig) -> Result<Client, tokio_postgres::Error> {
    let params = format!(
        "host={} port={} user={} password={} dbname={} connect_timeout=2",
        config.host, config.port, config.user, config.password, config.database
    );
    let (client, connection) = tokio_postgres::connect(&params, NoTls).await?;
    // The connection future drives the socket; without it the client is dead.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Postgres connection error: {}", e);
        }
    });
    Ok(client)
}

/// Keeps a working client in `state.db`: reconnect whenever the stored one is
/// missing or no longer answers.
async fn maintain_connection(state: AppState, config: ConnectionConfig) {
    let mut attempt: u32 = 0;
    loop {
        let alive = {
            let guard = state.db.lock().await;
            match guard.as_ref() {
                Some(client) => tokio::time::timeout(QUERY_TIMEOUT, client.simple_query("SELECT 1"))
                    .await
                    .map(|r| r.is_ok())
                    .unwrap_or(false),
                None => false,
            }
        };

        if alive {
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }

        *state.db.lock().await = None;
        attempt += 1;

        match connect(&config).await {
            Ok(client) => {
                let created = client
                    .batch_execute(
                        "CREATE TABLE IF NOT EXISTS pongs (\
                           id integer PRIMARY KEY, \
                           count integer NOT NULL DEFAULT 0\
                         )",
                    )
                    .await;
                match created {
                    Ok(_) => {
                        if let Err(e) = client
                            .execute(
                                "INSERT INTO pongs (id, count) VALUES (1, 0) \
                                 ON CONFLICT (id) DO NOTHING",
                                &[],
                            )
                            .await
                        {
                            eprintln!("ping-pong: seeding the counter failed: {}", e);
                        }
                        println!(
                            "{} ping-pong connected to {}:{}",
                            timestamp(),
                            config.host,
                            config.port
                        );
                        *state.db.lock().await = Some(client);
                        attempt = 0;
                    }
                    Err(e) => eprintln!("ping-pong: creating the table failed: {}", e),
                }
            }
            Err(e) => println!(
                "{} ping-pong: database not ready ({}) — retrying in 2s (attempt {})",
                timestamp(),
                e,
                attempt
            ),
        }

        tokio::time::sleep(RETRY_DELAY).await;
    }
}

/// GET /healthz → 200 only when the database answers.
async fn healthz(State(state): State<AppState>) -> (StatusCode, String) {
    let guard = state.db.lock().await;
    let reachable = match guard.as_ref() {
        Some(client) => tokio::time::timeout(QUERY_TIMEOUT, client.simple_query("SELECT 1"))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false),
        None => false,
    };
    println!(
        "{} /healthz -> {} ({})",
        timestamp(),
        if reachable { 200 } else { 500 },
        if reachable {
            "database reachable"
        } else {
            "no database connection"
        }
    );
    (
        if reachable {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        },
        if reachable { "ok".to_string() } else { "unhealthy".to_string() },
    )
}

/// GET / → increment the counter and answer "pong N".
async fn root(State(state): State<AppState>) -> (StatusCode, String) {
    let guard = state.db.lock().await;
    let Some(client) = guard.as_ref() else {
        println!("{} GET / -> 500 (no database connection)", timestamp());
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "unhealthy".to_string(),
        );
    };

    let result = tokio::time::timeout(
        QUERY_TIMEOUT,
        client.query_one(
            "UPDATE pongs SET count = count + 1 WHERE id = 1 RETURNING count",
            &[],
        ),
    )
    .await;

    match result {
        Ok(Ok(row)) => {
            let count: i32 = row.get(0);
            println!("{} GET / -> 200 (pong {})", timestamp(), count);
            (StatusCode::OK, format!("pong {}\n", count))
        }
        Ok(Err(e)) => {
            eprintln!("ping-pong: incrementing the counter failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "unhealthy".to_string())
        }
        Err(_) => {
            println!("{} GET / -> 500 (database timed out)", timestamp());
            (StatusCode::INTERNAL_SERVER_ERROR, "unhealthy".to_string())
        }
    }
}

fn timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}
