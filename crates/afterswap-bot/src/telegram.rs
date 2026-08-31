//! Minimal Telegram Bot API client, plus the `Sink` seam that lets the entire
//! bot run with no token.
//!
//! Long-polling `getUpdates` rather than a webhook: no public URL, no TLS
//! terminator, no deploy step between a judge and a working demo.

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Mutex;

/// Where a rendered message goes.
///
/// `StdoutSink` exists so `--dry-run` exercises the real watcher loop — same
/// engine, same DFlow quotes, same phrasing — without BotFather. Registering a
/// bot needs a human's Telegram account, so the demo path must not depend on
/// one having been registered.
pub trait Sink: Send + Sync {
    /// Deliver one message to a chat. Errors are logged by the caller and
    /// never abort a watch: a dropped message is worth less than the position.
    fn send(
        &self,
        chat_id: i64,
        text: String,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

/// Prints to stdout with the chat id prefixed.
pub struct StdoutSink;

impl Sink for StdoutSink {
    async fn send(&self, chat_id: i64, text: String) -> anyhow::Result<()> {
        println!("[chat {chat_id}] {text}");
        Ok(())
    }
}

/// One incoming message, reduced to the two fields the bot uses.
#[derive(Debug, Clone, PartialEq)]
pub struct Incoming {
    pub chat_id: i64,
    pub text: String,
}

#[derive(Deserialize)]
struct UpdatesResponse {
    ok: bool,
    #[serde(default)]
    result: Vec<Update>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    chat: Chat,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct Chat {
    id: i64,
}

/// Bot API client over `https://api.telegram.org/bot<token>`.
pub struct Telegram {
    http: reqwest::Client,
    base: String,
    /// Highest acknowledged update id. Telegram only drops an update once a
    /// later offset is requested, so this is the at-least-once cursor.
    offset: Arc<Mutex<i64>>,
}

impl Telegram {
    /// Client for `token`. The token is only ever placed in the URL, which is
    /// why nothing here logs a full request line.
    pub fn new(token: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: format!("https://api.telegram.org/bot{token}"),
            offset: Arc::new(Mutex::new(0)),
        }
    }

    /// One long poll. Returns whatever arrived within `timeout_secs`, which is
    /// commonly nothing.
    pub async fn poll(&self, timeout_secs: u64) -> anyhow::Result<Vec<Incoming>> {
        let offset = *self.offset.lock().await;
        let body: UpdatesResponse = self
            .http
            .get(format!("{}/getUpdates", self.base))
            .query(&[
                ("offset", offset.to_string()),
                ("timeout", timeout_secs.to_string()),
            ])
            // Telegram holds the request open for `timeout`; the read budget
            // has to exceed that or every poll ends in a client-side timeout.
            .timeout(std::time::Duration::from_secs(timeout_secs + 10))
            .send()
            .await?
            .json()
            .await?;

        if !body.ok {
            anyhow::bail!(
                "telegram getUpdates rejected: {}",
                body.description.unwrap_or_else(|| "no reason given".into())
            );
        }

        let mut out = Vec::new();
        let mut highest = offset;
        for update in body.result {
            highest = highest.max(update.update_id + 1);
            let Some(msg) = update.message else { continue };
            let Some(text) = msg.text else { continue };
            out.push(Incoming {
                chat_id: msg.chat.id,
                text,
            });
        }
        *self.offset.lock().await = highest;
        Ok(out)
    }
}

impl Sink for Telegram {
    async fn send(&self, chat_id: i64, text: String) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/sendMessage", self.base))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                "disable_web_page_preview": true,
            }))
            .send()
            .await?;
        match resp.status().is_success() {
            true => Ok(()),
            false => anyhow::bail!("telegram sendMessage failed: {}", resp.status()),
        }
    }
}
