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
        Config {
            rpc_ws_url: kairos_config::require_env("HELIUS_WS_URL"),
            rpc_http_url: kairos_config::require_env("HELIUS_RPC_URL"),
            program_id: kairos_config::devnet_program_id("perp"),
            lp_pool_program_id: kairos_config::devnet_program_id("liquidity_pool"),
            database_url: kairos_config::require_env("DATABASE_URL"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
