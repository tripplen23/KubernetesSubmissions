use chrono::Utc;
use tokio::time::{interval, Duration};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let random_string = Uuid::new_v4().to_string();
    let mut ticker = interval(Duration::from_secs(5));

    loop {
        ticker.tick().await;
        println!("{}: {}", Utc::now().to_rfc3339(), random_string);
    }
}
