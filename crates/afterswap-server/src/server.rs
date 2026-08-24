//! Axum server: dashboard static files + snapshot API + SSE tick stream.

use std::sync::Arc;

use afterswap_engine::ExitEngine;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use log::info;
use serde::Deserialize;
use tokio::sync::{Mutex, broadcast};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

/// Engine shared between the paper loop and HTTP handlers.
pub type SharedEngine = Arc<Mutex<ExitEngine>>;

/// Server-side app state.
#[derive(Clone)]
pub struct AppState {
    pub engine: SharedEngine,
    /// Full-snapshot JSON broadcast every tick.
    pub snapshots: broadcast::Sender<String>,
}

/// Number of recent prices included in each snapshot.
pub const SNAPSHOT_PRICES: usize = 240;

#[derive(Deserialize)]
struct OpenBody {
    size: f64,
}

async fn get_state(State(state): State<AppState>) -> impl IntoResponse {
    let engine = state.engine.lock().await;
    Json(engine.snapshot(SNAPSHOT_PRICES)).into_response()
}

async fn open_position(
    State(state): State<AppState>,
    Json(body): Json<OpenBody>,
) -> impl IntoResponse {
    let mut engine = state.engine.lock().await;
    match engine.open_position(body.size) {
        Some(pos) => {
            info!("position opened via API: {} @ {}", pos.size, pos.entry_price);
            Json(serde_json::json!({"ok": true, "entry_price": pos.entry_price}))
        }
        None => Json(serde_json::json!({"ok": false, "error": "no price yet"})),
    }
}

async fn close_position(State(state): State<AppState>) -> impl IntoResponse {
    let mut engine = state.engine.lock().await;
    let value = engine.close_position();
    Json(serde_json::json!({"ok": value.is_some(), "final_value_norm": value}))
}

async fn sse_events(
    State(state): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.snapshots.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(json) => Some(Ok(Event::default().event("snapshot").data(json))),
        Err(_) => None, // lagged receiver — next snapshot resyncs fully
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../web/index.html"))
}

/// Build the router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/state", get(get_state))
        .route("/api/events", get(sse_events))
        .route("/api/position/open", post(open_position))
        .route("/api/position/close", post(close_position))
        .with_state(state)
}

/// Serve on `port` until shutdown.
pub async fn serve(state: AppState, port: u16) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("dashboard on http://localhost:{port}");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
