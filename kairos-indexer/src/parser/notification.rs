use serde::Deserialize;

/// Every `logsNotification` websocket message carries a `Program data:` line for each
/// Anchor event it emits — this is that prefix, shared by every program's decoder.
pub const PROGRAM_DATA_PREFIX: &str = "Program data: ";

#[derive(Debug, Deserialize)]
pub struct LogsNotification {
    pub params: NotificationParams,
}

#[derive(Debug, Deserialize)]
pub struct NotificationParams {
    pub result: NotificationResult,
}

#[derive(Debug, Deserialize)]
pub struct NotificationResult {
    pub context: NotificationContext,
    pub value: NotificationValue,
}

#[derive(Debug, Deserialize)]
pub struct NotificationContext {
    pub slot: u64,
}

#[derive(Debug, Deserialize)]
pub struct NotificationValue {
    pub signature: String,
    pub logs: Vec<String>,
}
