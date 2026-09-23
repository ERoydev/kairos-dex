use std::env;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::OnceLock;

use solana_sdk::pubkey::Pubkey;

const DEPLOYED_PROGRAMS_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../programs/deployed_programs.json"
);

/// Process-wide `OnceLock<T>` with panic-on-misuse `init`/`get`, so each service's `Config`
/// doesn't have to hand-roll the same static + init-once dance.
pub struct Global<T> {
    cell: OnceLock<T>,
}

impl<T> Global<T> {
    pub const fn new() -> Self {
        Self {
            cell: OnceLock::new(),
        }
    }

    /// Publishes `value` as the process-wide instance. Must be called exactly once, before
    /// any `get()` call — typically right after building `Config` in `main`.
    pub fn init(&self, value: T) {
        self.cell
            .set(value)
            .unwrap_or_else(|_| panic!("Global::init called more than once"));
    }

    /// Returns the process-wide instance set by `init`. Panics if that hasn't run yet.
    pub fn get(&self) -> &T {
        self.cell
            .get()
            .expect("Global::init must be called before Global::get()")
    }
}

impl<T> Default for Global<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads an env var, panicking with the var's name if it's missing.
pub fn require_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("{key} must be set"))
}

/// Reads an env var and parses it, panicking with the var's name if it's missing or invalid.
pub fn parse_env<T: FromStr>(key: &str) -> T
where
    T::Err: Display,
{
    require_env(key)
        .parse()
        .unwrap_or_else(|e| panic!("{key} is not valid: {e}"))
}

fn deployed_programs() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(DEPLOYED_PROGRAMS_JSON)
            .unwrap_or_else(|e| panic!("failed to read {DEPLOYED_PROGRAMS_JSON}: {e}")),
    )
    .unwrap_or_else(|e| panic!("failed to parse {DEPLOYED_PROGRAMS_JSON}: {e}"))
}

/// Looks up `programs/deployed_programs.json` for `program_name`'s address on `cluster`
/// (e.g. `"perp"`, `"devnet"`). Panics if the program or cluster entry is missing/invalid.
pub fn deployed_program_id(program_name: &str, cluster: &str) -> Pubkey {
    let deployed = deployed_programs();
    let raw = deployed[program_name][cluster].as_str().unwrap_or_else(|| {
        panic!("no {cluster} id for '{program_name}' in {DEPLOYED_PROGRAMS_JSON}")
    });
    Pubkey::from_str(raw).unwrap_or_else(|e| panic!("invalid {program_name} id '{raw}': {e}"))
}

/// Shorthand for `deployed_program_id(program_name, "devnet")`.
pub fn devnet_program_id(program_name: &str) -> Pubkey {
    deployed_program_id(program_name, "devnet")
}

/// Every program in `programs/deployed_programs.json` that has a devnet address.
pub fn devnet_program_ids() -> Vec<Pubkey> {
    let deployed = deployed_programs();
    let Some(programs) = deployed.as_object() else {
        return Vec::new();
    };

    programs
        .values()
        .filter_map(|entry| entry["devnet"].as_str())
        .map(|raw| Pubkey::from_str(raw).unwrap_or_else(|e| panic!("invalid program id '{raw}': {e}")))
        .collect()
}
