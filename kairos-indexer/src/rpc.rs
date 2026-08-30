use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::json;
use solana_sdk::pubkey::Pubkey;

use crate::parser::accounts::{AccountDecodeError, AnchorAccount, decode_account};

/// Minimal Solana JSON-RPC HTTP client — just the one call the indexer needs
/// (`getAccountInfo`) to enrich events with on-chain account state they don't carry.
///
/// Cheap to `Clone`: `reqwest::Client` pools connections internally behind an `Arc`, so cloning
/// this (e.g. to share across multiple `Subscriber`s hitting the same RPC URL) reuses the pool
/// instead of opening a second one.
#[derive(Clone)]
pub struct RpcClient {
    http: reqwest::Client,
    url: String,
}

impl RpcClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.into(),
        }
    }

    /// Fetches an account and decodes it as `T`. Returns `Ok(None)` if the account doesn't
    /// exist on-chain (e.g. it was already closed by the time we asked).
    pub async fn get_account<T: AnchorAccount>(
        &self,
        pubkey: &Pubkey,
    ) -> Result<Option<T>, RpcError> {
        let Some(data) = self.get_account_data(pubkey).await? else {
            return Ok(None);
        };

        decode_account(&data).map(Some).map_err(RpcError::Decode)
    }

    async fn get_account_data(&self, pubkey: &Pubkey) -> Result<Option<Vec<u8>>, RpcError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [pubkey.to_string(), { "encoding": "base64", "commitment": "confirmed" }]
        });

        let response: GetAccountInfoResponse = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        let Some(value) = response.result.value else {
            return Ok(None);
        };

        BASE64
            .decode(&value.data.0)
            .map(Some)
            .map_err(RpcError::InvalidBase64)
    }
}

#[derive(Debug)]
pub enum RpcError {
    Http(reqwest::Error),
    InvalidBase64(base64::DecodeError),
    Decode(AccountDecodeError),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "RPC request failed: {e}"),
            Self::InvalidBase64(e) => write!(f, "invalid base64 account data: {e}"),
            Self::Decode(e) => write!(f, "failed to decode account: {e}"),
        }
    }
}

impl From<reqwest::Error> for RpcError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

#[derive(Debug, Deserialize)]
struct GetAccountInfoResponse {
    result: RpcResult,
}

#[derive(Debug, Deserialize)]
struct RpcResult {
    value: Option<AccountInfoValue>,
}

#[derive(Debug, Deserialize)]
struct AccountInfoValue {
    /// `[data, encoding]` — we always request `"base64"` encoding.
    data: (String, String),
}
