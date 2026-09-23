use std::env;

use kairos_config::Global;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, read_keypair_file};

#[allow(unused)]
pub struct Config {
    pub rpc_url: String,
    pub database_url: String,
    pub perp_program_id: Pubkey,
    pub usdc_mint: Pubkey,
    pub keeper_keypair: Keypair,
    pub liquidation_poll_interval_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            rpc_url: kairos_config::require_env("RPC_URL"),
            database_url: kairos_config::require_env("DATABASE_URL"),
            perp_program_id: kairos_config::devnet_program_id("perp"),
            usdc_mint: kairos_config::parse_env("USDC_MINT"),
            keeper_keypair: load_keeper_keypair(),
            liquidation_poll_interval_secs: kairos_config::parse_env(
                "LIQUIDATION_POLL_INTERVAL_SECS",
            ),
        }
    }

    /// Publishes `self` as the process-wide config. Must be called exactly once, before any
    /// `config::get()` call — `main` does this right after `Config::from_env()`.
    pub fn init(self) {
        CONFIG.init(self);
    }
}

static CONFIG: Global<Config> = Global::new();

/// Returns the process-wide config set by `Config::init`. Panics if that hasn't run yet.
pub fn get() -> &'static Config {
    CONFIG.get()
}

/// Exactly one of `KEEPER_KEYPAIR_PATH` / `KEEPER_KEYPAIR_SECRET` must be set (see `.env.example`).
fn load_keeper_keypair() -> Keypair {
    match (
        env::var("KEEPER_KEYPAIR_PATH"),
        env::var("KEEPER_KEYPAIR_SECRET"),
    ) {
        (Ok(path), Err(_)) => read_keypair_file(&path)
            .unwrap_or_else(|e| panic!("failed to read keypair at `{path}`: {e}")),
        (Err(_), Ok(secret)) => Keypair::from_base58_string(&secret),
        (Ok(_), Ok(_)) => {
            panic!("set only one of KEEPER_KEYPAIR_PATH or KEEPER_KEYPAIR_SECRET, not both")
        }
        (Err(_), Err(_)) => panic!("set one of KEEPER_KEYPAIR_PATH or KEEPER_KEYPAIR_SECRET"),
    }
}
