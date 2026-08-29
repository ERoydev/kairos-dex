use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

pub struct Config {
    pub rpc_ws_url: String,
    pub rpc_http_url: String,
    pub program_id: Pubkey,
    pub lp_pool_program_id: Pubkey,
    pub database_url: String,
}

impl Config {
    pub fn new() -> Config {
        let program_id = parse_pubkey_env("PERP_PROGRAM_ID");
        let lp_pool_program_id = parse_pubkey_env("LP_POOL_PROGRAM_ID");

        Config {
            rpc_ws_url: std::env::var("HELIUS_WS_URL").expect("missing HELIUS_WS_URL"),
            rpc_http_url: std::env::var("HELIUS_RPC_URL").expect("missing HELIUS_RPC_URL"),
            program_id,
            lp_pool_program_id,
            database_url: std::env::var("DATABASE_URL").expect("missing DATABASE_URL"),
        }
    }
}

fn parse_pubkey_env(var: &str) -> Pubkey {
    let raw = std::env::var(var).unwrap_or_else(|_| panic!("missing {var}"));
    Pubkey::from_str(&raw).unwrap_or_else(|e| panic!("invalid {var} '{raw}': {e}"))
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
