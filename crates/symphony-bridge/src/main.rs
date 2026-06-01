//! Symphony bridge — streams infrastructure telemetry as musical/visual frames.

use anyhow::{Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, RwLock},
    task::JoinHandle,
};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "symphony-bridge", about = "Infrastructure metrics → Symphony WebSocket bridge")]
struct Args {
    /// HTTP listen address (UI + WebSocket)
    #[arg(long, env = "SYMPHONY_HTTP_ADDR", default_value = "0.0.0.0:8765")]
    http_addr: SocketAddr,

    /// Directory containing static UI assets
    #[arg(long, env = "SYMPHONY_WEB_DIR", default_value = "web")]
    web_dir: PathBuf,

    /// Run built-in demo metrics (ignores NATS when no URL)
    #[arg(long, env = "SYMPHONY_DEMO", default_value_t = true)]
    demo: bool,

    /// NATS URL (optional — subscribes to Nexus event subjects)
    #[arg(long, env = "NATS_URL")]
    nats_url: Option<String>,

    /// Comma-separated NATS subjects
    #[arg(
        long,
        env = "SYMPHONY_NATS_SUBJECTS",
        default_value = "nexus.anomalies.>,nexus.chaos.>,nexus.feedback.>"
    )]
    nats_subjects: String,

    /// Anomaly scorer base URL (optional HTTP poll)
    #[arg(long, env = "SYMPHONY_ANOMALY_URL")]
    anomaly_url: Option<String>,

    /// Tick interval for demo / synthesis loop (ms)
    #[arg(long, env = "SYMPHONY_TICK_MS", default_value_t = 100)]
    tick_ms: u64,
}

/// Single frame consumed by the browser sonification engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymphonyFrame {
    pub ts_ms: u64,
    pub target: String,
    pub request_rate: f64,
    pub latency_p99_ms: f64,
    pub error_rate: f64,
    pub anomaly_score: f64,
    pub chaos: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<SymphonyFrame>,
    metrics: Arc<RwLock<LiveMetrics>>,
}

#[derive(Debug, Clone, Default)]
struct LiveMetrics {
    target: String,
    request_rate: f64,
    latency_p99_ms: f64,
    error_rate: f64,
    anomaly_score: f64,
    chaos_pulse: bool,
    event: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let (tx, _) = broadcast::channel(256);
    let state = AppState {
        tx: tx.clone(),
        metrics: Arc::new(RwLock::new(LiveMetrics {
            target: "production/api".into(),
            request_rate: 800.0,
            latency_p99_ms: 48.0,
            error_rate: 0.002,
            anomaly_score: 0.3,
            ..Default::default()
        })),
    };

    let tick_ms = args.tick_ms.max(50);
    let mut background_tasks: Vec<JoinHandle<()>> = Vec::new();

    background_tasks.push(spawn_broadcaster(state.clone(), tick_ms));

    if args.demo || args.nats_url.is_none() {
        background_tasks.push(spawn_demo_driver(state.clone(), tick_ms));
    }
    if let Some(url) = args.nats_url.clone() {
        let subjects: Vec<String> = args
            .nats_subjects
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        background_tasks.push(spawn_nats_ingest(state.clone(), url, subjects));
    }
    if let Some(base) = args.anomaly_url.clone() {
        background_tasks.push(spawn_anomaly_poll(state.clone(), base));
    }

    tracing::info!(
        workers = background_tasks.len(),
        "symphony background workers started"
    );

    let web_dir = args
        .web_dir
        .canonicalize()
        .unwrap_or_else(|_| args.web_dir.clone());
    let index = web_dir.join("index.html");
    if !index.exists() {
        anyhow::bail!(
            "symphony web assets not found at {} (set SYMPHONY_WEB_DIR)",
            web_dir.display()
        );
    }

    let static_files = ServeDir::new(&web_dir).not_found_service(ServeFile::new(index));

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .nest_service("/", static_files)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(%args.http_addr, web = %web_dir.display(), "symphony bridge listening");
    let listener = tokio::net::TcpListener::bind(args.http_addr).await?;
    axum::serve(listener, app).await?;

    for task in background_tasks {
        task.abort();
    }
    Ok(())
}

async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok", "service": "symphony-bridge" }))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(frame) => {
                    let text = match serde_json::to_string(&frame) {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::warn!(%e, "serialize frame");
                            continue;
                        }
                    };
                    if sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::debug!(lagged = n, "ws client lagged");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

fn spawn_broadcaster(state: AppState, tick_ms: u64) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
        loop {
            interval.tick().await;
            let m = state.metrics.read().await.clone();
            let chaos = m.chaos_pulse;
            let frame = SymphonyFrame {
                ts_ms: now_ms(),
                target: m.target,
                request_rate: m.request_rate,
                latency_p99_ms: m.latency_p99_ms,
                error_rate: m.error_rate,
                anomaly_score: m.anomaly_score,
                chaos,
                event: m.event.clone(),
            };
            if chaos {
                let mut w = state.metrics.write().await;
                w.chaos_pulse = false;
                w.event = None;
            }
            let _ = state.tx.send(frame);
        }
    })
}

fn spawn_demo_driver(state: AppState, tick_ms: u64) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut t: f64 = 0.0;
        let step = tick_ms as f64 / 1000.0;
        let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
        tracing::info!("demo metrics driver active (set SYMPHONY_DEMO=false to disable)");
        loop {
            interval.tick().await;
            t += step;
            let incident = ((t * 0.07).sin() + 1.0) / 2.0;
            let spike = ((t * 0.31).sin() * (t * 0.11).cos()).abs();
            let mut w = state.metrics.write().await;
            w.target = if (t as i64) % 40 < 20 {
                "production/api".into()
            } else {
                "staging/workers".into()
            };
            w.request_rate = 400.0 + 900.0 * (1.0 + (t * 0.5).sin()) + spike * 600.0;
            w.latency_p99_ms = 35.0 + 120.0 * incident + spike * 180.0;
            w.error_rate = (0.0005 + 0.02 * spike).min(0.08);
            w.anomaly_score = (incident * 2.5 + spike * 3.0).min(5.0);
            if spike > 0.92 && (t * 10.0) as i64 % 7 == 0 {
                w.chaos_pulse = true;
                w.event = Some("demo_chaos_pulse".into());
            }
        }
    })
}

fn spawn_nats_ingest(state: AppState, url: String, subjects: Vec<String>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match run_nats_ingest(state.clone(), &url, &subjects).await {
                Ok(()) => tracing::warn!("nats ingest ended, reconnecting"),
                Err(e) => tracing::warn!(%e, "nats ingest error, retry in 3s"),
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    })
}

async fn run_nats_ingest(state: AppState, url: &str, subjects: &[String]) -> Result<()> {
    let client = async_nats::connect(url)
        .await
        .with_context(|| format!("connect nats at {url}"))?;
    tracing::info!(%url, ?subjects, "nats ingest connected");

    let mut subscriber_tasks = Vec::new();
    for subject in subjects {
        let mut sub = client.subscribe(subject.clone()).await?;
        let state = state.clone();
        let subject_name = subject.clone();
        subscriber_tasks.push(tokio::spawn(async move {
            while let Some(msg) = sub.next().await {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&msg.payload) {
                    apply_nats_event(&state, &subject_name, &v).await;
                }
            }
        }));
    }

    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn apply_nats_event(state: &AppState, subject: &str, value: &serde_json::Value) {
    let mut w = state.metrics.write().await;
    if subject.contains("chaos") {
        w.chaos_pulse = true;
        w.error_rate = (w.error_rate + 0.01).min(0.15);
        w.event = value
            .get("experiment")
            .or_else(|| value.get("fault"))
            .and_then(|v| v.as_str())
            .map(|s| format!("chaos:{s}"));
        return;
    }
    if subject.contains("anomal") {
        if let Some(score) = value.get("score").and_then(|v| v.as_f64()) {
            w.anomaly_score = score;
        }
        w.latency_p99_ms = (w.latency_p99_ms * 1.05).min(500.0);
        w.event = Some("anomaly".into());
        return;
    }
    if subject.contains("feedback") {
        if let Some(target) = value.get("target").and_then(|v| v.as_str()) {
            w.target = target.into();
        }
        w.event = Some("feedback".into());
    }
}

fn spawn_anomaly_poll(state: AppState, base: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!("{}/v1/score", base.trim_end_matches('/'));
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        tracing::info!(%url, "polling anomaly scorer");
        loop {
            interval.tick().await;
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if let Some(score) = body.get("score").and_then(|v| v.as_f64()) {
                            state.metrics.write().await.anomaly_score = score;
                        }
                    }
                }
                Ok(resp) => tracing::debug!(status = %resp.status(), "anomaly poll non-200"),
                Err(e) => tracing::debug!(%e, "anomaly poll failed"),
            }
        }
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip_json() {
        let frame = SymphonyFrame {
            ts_ms: 1,
            target: "production/api".into(),
            request_rate: 1000.0,
            latency_p99_ms: 42.0,
            error_rate: 0.01,
            anomaly_score: 1.5,
            chaos: false,
            event: None,
        };
        let j = serde_json::to_string(&frame).unwrap();
        let back: SymphonyFrame = serde_json::from_str(&j).unwrap();
        assert_eq!(back.target, "production/api");
    }
}
