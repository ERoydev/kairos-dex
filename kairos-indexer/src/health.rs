use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;

const STALE_AFTER_SECS: i64 = 60;

pub struct HealthState {
    connected: AtomicBool,
    last_event_at: AtomicI64,
}

impl HealthState {
    pub fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            last_event_at: AtomicI64::new(0),
        }
    }

    pub fn mark_connected(&self) {
        self.connected.store(true, Ordering::Relaxed);
    }

    pub fn mark_disconnected(&self) {
        self.connected.store(false, Ordering::Relaxed);
    }

    pub fn mark_event_processed(&self) {
        self.last_event_at.store(now(), Ordering::Relaxed);
    }
}

impl Default for HealthState {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub async fn health_handler(
    State(state): State<Arc<HealthState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let connected = state.connected.load(Ordering::Relaxed);
    let last_event_at = state.last_event_at.load(Ordering::Relaxed);
    let stale = last_event_at != 0 && now() - last_event_at > STALE_AFTER_SECS;
    let healthy = connected && !stale;

    let body = json!({
        "status": if healthy { "ok" } else { "degraded" },
        "connected": connected,
        "last_event_at": last_event_at,
    });

    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(body))
}
