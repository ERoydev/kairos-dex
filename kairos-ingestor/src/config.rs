use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::OnceLock;

use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;

use crate::Result;

const DEPLOYED_PROGRAMS_JSON: &str = include_str!("../../programs/deployed_programs.json");

#[derive(Deserialize)]
struct DeployedProgram {
    devnet: Option<String>,
}

/// Reads `programs/deployed_programs.json` (embedded at compile time) and returns the
/// devnet address of every program that has one deployed.
pub fn load_devnet_program_ids() -> Result<Vec<Pubkey>> {
    let programs: HashMap<String, DeployedProgram> = serde_json::from_str(DEPLOYED_PROGRAMS_JSON)?;

    programs
        .into_values()
        .filter_map(|program| program.devnet)
        .map(|address| Pubkey::from_str(&address).map_err(Into::into))
        .collect()
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_ws_url: String,
    pub rpc_http_url: String,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            rpc_ws_url: require_env("HELIUS_WS_URL"),
            rpc_http_url: require_env("HELIUS_RPC_URL"),
            database_url: require_env("DATABASE_URL"),
        }
    }

    /// Publishes `self` as the process-wide config. Must be called exactly once, before any
    /// `config::get()` call — `main` does this right after `Config::from_env()`.
    pub fn init(self) {
        CONFIG
            .set(self)
            .unwrap_or_else(|_| panic!("Config::init called more than once"));
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns the process-wide config set by `Config::init`. Panics if that hasn't run yet — every
/// task spawned after `main` calls it can rely on this instead of having `Config` threaded
/// through as a parameter.
pub fn get() -> &'static Config {
    CONFIG
        .get()
        .expect("Config::init must be called before config::get()")
}

fn require_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("{key} must be set"))
}
