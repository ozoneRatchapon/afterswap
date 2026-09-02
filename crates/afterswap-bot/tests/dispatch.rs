//! End-to-end command handling with a capturing sink: real registry, real
//! phrasing, no Telegram and no DFlow. Anything that would poll a quote is
//! exercised through the commands that do not open a watch.

use std::sync::{Arc, Mutex};

use afterswap_bot::dispatch::{Registry, handle};
use afterswap_bot::phrase::Verbosity;
use afterswap_bot::telegram::{Incoming, Sink};

#[derive(Default)]
struct CaptureSink {
    sent: Mutex<Vec<(i64, String)>>,
}

impl CaptureSink {
    fn drain(&self) -> Vec<(i64, String)> {
        std::mem::take(&mut *self.sent.lock().unwrap())
    }
}

impl Sink for CaptureSink {
    async fn send(&self, chat_id: i64, text: String) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push((chat_id, text));
        Ok(())
    }
}

async fn say(sink: &Arc<CaptureSink>, registry: &Registry, text: &str) -> Vec<String> {
    let msg = Incoming {
        chat_id: 42,
        text: text.to_string(),
    };
    handle(&msg, registry, 1_000, Verbosity::Quiet, sink.clone())
        .await
        .expect("capture sink never fails");
    sink.drain().into_iter().map(|(_, t)| t).collect()
}

#[tokio::test]
async fn start_explains_the_product_and_lists_the_commands() {
    let sink = Arc::new(CaptureSink::default());
    let registry = Registry::new();
    let out = say(&sink, &registry, "/start").await;
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("/watch SOL"), "{}", out[0]);
    assert!(out[0].contains("/status"), "{}", out[0]);
    assert!(out[0].contains("Paper mode"), "{}", out[0]);
}

#[tokio::test]
async fn chatter_gets_no_reply_at_all() {
    let sink = Arc::new(CaptureSink::default());
    let registry = Registry::new();
    assert!(say(&sink, &registry, "gm ser").await.is_empty());
}

#[tokio::test]
async fn status_and_stop_are_safe_before_any_watch_exists() {
    let sink = Arc::new(CaptureSink::default());
    let registry = Registry::new();

    let out = say(&sink, &registry, "/status").await;
    assert!(out[0].contains("/watch SOL"), "{}", out[0]);

    let out = say(&sink, &registry, "/stop").await;
    assert!(out[0].contains("Nothing to stop"), "{}", out[0]);
    assert!(!registry.is_watching(42).await);
}

#[tokio::test]
async fn a_bad_watch_never_registers_a_watch() {
    let sink = Arc::new(CaptureSink::default());
    let registry = Registry::new();
    for bad in ["/watch", "/watch DOGE", "/watch SOL later"] {
        let out = say(&sink, &registry, bad).await;
        assert_eq!(out.len(), 1, "{bad}");
        assert!(
            !registry.is_watching(42).await,
            "{bad} must not start a watch"
        );
    }
}

#[tokio::test]
async fn proof_is_reachable_without_ever_opening_a_position() {
    // The evidence has to be readable by a judge who never runs the demo.
    let sink = Arc::new(CaptureSink::default());
    let registry = Registry::new();
    let out = say(&sink, &registry, "/proof").await;
    assert!(out[0].contains("could not prove"), "{}", out[0]);
    assert!(
        out[0].contains("github.com/ozoneRatchapon/afterswap"),
        "{}",
        out[0]
    );
}
