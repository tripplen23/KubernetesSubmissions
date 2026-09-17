use futures::StreamExt;
use serde_json::Value;
use std::env;
use std::time::Duration;

/// Optional env var with a default — the broadcaster must start even if the
/// Deployment leaves a value out.
fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

/// NATS is a separate Deployment (in the lab it is a Helm release), so it can be
/// a second behind us. Retry for a while instead of crash-looping.
async fn connect_with_retry(url: &str) -> async_nats::Client {
    let mut attempt = 0u32;
    loop {
        match async_nats::connect(url).await {
            Ok(client) => {
                println!("connected to NATS at {url}");
                return client;
            }
            Err(e) => {
                attempt += 1;
                if attempt >= 30 {
                    panic!("NATS not reachable after {attempt} attempts: {e}");
                }
                eprintln!("NATS not ready ({e}) — retrying in 2s ({attempt}/30)…");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Turn the event the backend published into the sentence a chat service shows.
fn format_message(raw: &str) -> String {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => {
            let event = value.get("event").and_then(|e| e.as_str()).unwrap_or("event");
            let id = value.pointer("/todo/id").and_then(|i| i.as_i64()).unwrap_or(0);
            let title = value
                .pointer("/todo/title")
                .and_then(|t| t.as_str())
                .unwrap_or("(no title)");
            match event {
                "created" => format!("A todo was created: #{id} {title}"),
                "done" => format!("A todo was marked done: #{id} {title}"),
                "undone" => format!("A todo was marked undone: #{id} {title}"),
                "deleted" => format!("A todo was deleted: #{id} {title}"),
                other => format!("{other}: #{id} {title}"),
            }
        }
        Err(_) => format!("unreadable event: {raw}"),
    }
}

/// The chat service's payload shape. The exercise lets you pick a service and
/// send "the message […] in a format they support", and the formats are not the
/// same shape:
///
/// - `discord` — an incoming webhook takes `{"content": "…"}`
/// - `slack`   — an incoming webhook takes `{"text": "…"}`
/// - `generic` — the exercise's third option: `{"user": "bot", "message": "…"}`
///
/// Telegram is not here on purpose: its Bot API wants a token, a chat id and a
/// `sendMessage` call, which is a different integration, not a payload tweak.
fn format_payload(chat_format: &str, bot_name: &str, text: String) -> serde_json::Value {
    match chat_format {
        "discord" => serde_json::json!({ "content": text }),
        "slack" => serde_json::json!({ "text": text }),
        "generic" => serde_json::json!({ "user": bot_name, "message": text }),
        other => {
            eprintln!("unknown CHAT_FORMAT '{other}' — falling back to generic");
            serde_json::json!({ "user": bot_name, "message": text })
        }
    }
}

#[tokio::main]
async fn main() {
    let nats_url = env_or("NATS_URL", "nats://my-nats:4222");
    let subject = env_or("NATS_SUBJECT", "todo_events");
    let queue_group = env_or("NATS_QUEUE_GROUP", "broadcasters");
    let chat_url = env_or("CHAT_URL", "http://chat-sink:8080");
    let bot_name = env_or("BOT_NAME", "bot");
    let chat_format = env_or("CHAT_FORMAT", "generic");

    let nats = connect_with_retry(&nats_url).await;

    // The queue group is the whole exercise. Six replicas subscribing to the same
    // subject *inside the same group* means NATS hands each message to exactly one
    // of them: six replicas, one delivery. Subscribe without a group — an empty
    // NATS_QUEUE_GROUP — and every replica receives every message, which is the
    // duplicate the exercise forbids. The lab runs both, to see the difference.
    let subscription = if queue_group.is_empty() {
        println!("listening on {subject} (NO queue group: every replica gets every message), forwarding to {chat_url}");
        nats.subscribe(subject.clone())
            .await
            .unwrap_or_else(|e| panic!("cannot subscribe to {subject}: {e}"))
    } else {
        println!("listening on {subject} (queue group {queue_group}), forwarding to {chat_url}");
        nats.queue_subscribe(subject.clone(), queue_group.clone())
            .await
            .unwrap_or_else(|e| panic!("cannot subscribe to {subject}: {e}"))
    };
    let mut subscription = subscription;

    let http = reqwest::Client::new();
    let mut forwarded: u64 = 0;

    while let Some(message) = subscription.next().await {
        let raw = String::from_utf8_lossy(&message.payload).to_string();
        let payload = format_payload(&chat_format, &bot_name, format_message(&raw));

        // Core NATS is at-most-once: if this fails, that one message is gone.
        // The exercise accepts a missing message; a duplicate is the problem.
        match http.post(&chat_url).json(&payload).send().await {
            Ok(response) if response.status().is_success() => {
                forwarded += 1;
                println!("[forward] #{forwarded} {payload}");
            }
            Ok(response) => {
                eprintln!("[forward] chat service answered {} for {payload}", response.status());
            }
            Err(e) => {
                eprintln!("[forward] chat service unreachable ({e}) for {payload}");
            }
        }
    }

    eprintln!("subscription to {subject} ended");
}