use std::env;
use std::str::FromStr;
use std::sync::OnceLock;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, read_keypair_file};

const DEPLOYED_PROGRAMS_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../programs/deployed_programs.json"
);

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
        let perp_program_id = Config::get_smart_contract_id();

        Self {
            rpc_url: require_env("RPC_URL"),
            database_url: require_env("DATABASE_URL"),
            perp_program_id,
            usdc_mint: parse_env("USDC_MINT"),
            keeper_keypair: load_keeper_keypair(),
            liquidation_poll_interval_secs: parse_env("LIQUIDATION_POLL_INTERVAL_SECS"),
        }
    }

    /// Publishes `self` as the process-wide config. Must be called exactly once, before any
    /// `config::get()` call — `main` does this right after `Config::from_env()`.
    pub fn init(self) {
        CONFIG
            .set(self)
            .unwrap_or_else(|_| panic!("Config::init called more than once"));
    }

    /// Get smart contract deployed program on devnet, from the json file stored
    fn get_smart_contract_id() -> Pubkey {
        let deployed: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(DEPLOYED_PROGRAMS_JSON)
                .unwrap_or_else(|e| panic!("failed to read {DEPLOYED_PROGRAMS_JSON}: {e}")),
        )
        .unwrap_or_else(|e| panic!("failed to parse {DEPLOYED_PROGRAMS_JSON}: {e}"));

        parse_pubkey_json(&deployed, "perp")
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
        (Ok(_), Ok(_)) => {
            panic!("set only one of KEEPER_KEYPAIR_PATH or KEEPER_KEYPAIR_SECRET, not both")
        }
        (Err(_), Err(_)) => panic!("set one of KEEPER_KEYPAIR_PATH or KEEPER_KEYPAIR_SECRET"),
    }
}

fn parse_pubkey_json(deployed: &serde_json::Value, program_name: &str) -> Pubkey {
    let raw = deployed[program_name]["devnet"]
        .as_str()
        .unwrap_or_else(|| panic!("no devnet id for '{program_name}' in {DEPLOYED_PROGRAMS_JSON}"));
    Pubkey::from_str(raw).unwrap_or_else(|e| panic!("invalid {program_name} id '{raw}': {e}"))
}
