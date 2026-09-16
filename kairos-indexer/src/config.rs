use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;

const DEPLOYED_PROGRAMS_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../programs/deployed_programs.json"
);

pub struct Config {
    pub rpc_ws_url: String,
    pub rpc_http_url: String,
    pub program_id: Pubkey,
    pub lp_pool_program_id: Pubkey,
    pub database_url: String,
}

impl Config {
    pub fn new() -> Config {
        let (program_id, lp_pool_program_id) = Config::get_smart_contract_ids();

        Config {
            rpc_ws_url: std::env::var("HELIUS_WS_URL").expect("missing HELIUS_WS_URL"),
            rpc_http_url: std::env::var("HELIUS_RPC_URL").expect("missing HELIUS_RPC_URL"),
            program_id,
            lp_pool_program_id,
            database_url: std::env::var("DATABASE_URL").expect("missing DATABASE_URL"),
        }
    }

    /// Get smart contract deployed program on devnet, from the json file stored
    fn get_smart_contract_ids() -> (Pubkey, Pubkey) {
        let deployed: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(DEPLOYED_PROGRAMS_JSON)
                .unwrap_or_else(|e| panic!("failed to read {DEPLOYED_PROGRAMS_JSON}: {e}")),
        )
        .unwrap_or_else(|e| panic!("failed to parse {DEPLOYED_PROGRAMS_JSON}: {e}"));

        let program_id = parse_pubkey_json(&deployed, "perp");
        let lp_pool_program_id = parse_pubkey_json(&deployed, "liquidity_pool");

        (program_id, lp_pool_program_id)
    }
}

fn parse_pubkey_json(deployed: &serde_json::Value, program_name: &str) -> Pubkey {
    let raw = deployed[program_name]["devnet"]
        .as_str()
        .unwrap_or_else(|| panic!("no devnet id for '{program_name}' in {DEPLOYED_PROGRAMS_JSON}"));
    Pubkey::from_str(raw).unwrap_or_else(|e| panic!("invalid {program_name} id '{raw}': {e}"))
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
