use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

pub struct Config {
    pub rpc_ws_url: String,
    pub program_id: Pubkey,
    pub database_url: String,
}

impl Config {
    pub fn new() -> Config {
        let raw_program_id_from_env =
            std::env::var("PERP_PROGRAM_ID").expect("missing PERP_PROGRAM_ID");

        let program_id = Pubkey::from_str(&raw_program_id_from_env)
            .unwrap_or_else(|e| panic!("invalid PERP_PROGRAM_ID '{raw_program_id_from_env}': {e}"));

        Config {
            rpc_ws_url: std::env::var("HELIUS_WS_URL").expect("missing HELIUS_WS_URL"),
            program_id,
            database_url: std::env::var("DATABASE_URL").expect("missing DATABASE_URL"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
