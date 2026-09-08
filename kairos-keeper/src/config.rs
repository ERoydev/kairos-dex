use std::env;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Keypair};

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
            rpc_url: require_env("RPC_URL"),
            database_url: require_env("DATABASE_URL"),
            perp_program_id: parse_env("PERP_PROGRAM_ID"),
            usdc_mint: parse_env("USDC_MINT"),
            keeper_keypair: load_keeper_keypair(),
            liquidation_poll_interval_secs: parse_env("LIQUIDATION_POLL_INTERVAL_SECS"),
        }
    }
}

fn require_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("{key} must be set"))
}

fn parse_env<T: std::str::FromStr>(key: &str) -> T
where
    T::Err: std::fmt::Display,
{
    require_env(key)
        .parse()
        .unwrap_or_else(|e| panic!("{key} is not valid: {e}"))
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
        (Ok(_), Ok(_)) => panic!(
            "set only one of KEEPER_KEYPAIR_PATH or KEEPER_KEYPAIR_SECRET, not both"
        ),
        (Err(_), Err(_)) => panic!(
            "set one of KEEPER_KEYPAIR_PATH or KEEPER_KEYPAIR_SECRET"
        ),
    }
}
