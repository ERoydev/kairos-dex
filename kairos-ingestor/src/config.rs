use kairos_config::Global;
use solana_sdk::pubkey::Pubkey;

/// Every program in `programs/deployed_programs.json` that has a devnet address.
pub fn load_devnet_program_ids() -> Vec<Pubkey> {
    kairos_config::devnet_program_ids()
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
            rpc_ws_url: kairos_config::require_env("HELIUS_WS_URL"),
            rpc_http_url: kairos_config::require_env("HELIUS_RPC_URL"),
            database_url: kairos_config::require_env("DATABASE_URL"),
        }
    }

    /// Publishes `self` as the process-wide config. Must be called exactly once, before any
    /// `config::get()` call — `main` does this right after `Config::from_env()`.
    pub fn init(self) {
        CONFIG.init(self);
    }
}

static CONFIG: Global<Config> = Global::new();

/// Returns the process-wide config set by `Config::init`. Panics if that hasn't run yet — every
/// task spawned after `main` calls it can rely on this instead of having `Config` threaded
/// through as a parameter.
pub fn get() -> &'static Config {
    CONFIG.get()
}
