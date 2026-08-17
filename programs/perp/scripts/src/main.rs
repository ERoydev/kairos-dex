// use anchor_client::{Client, Cluster, Signer};
// use anchor_lang::prelude::*;
// use serde::Deserialize;
// use solana_sdk::signer::keypair::Keypair;
// use std::{env, println, rc::Rc};

// declare_program!(perp);

// #[derive(Deserialize, Debug)]
// struct MarketConfig {
//     symbol: String,
//     oracle: String,
//     max_leverage: u64,
//     max_open_interest: u64,
//     maintenance_margin_bps: u64,
//     open_fee_bps: u64,
//     close_fee_bps: u64,
//     is_initialized: bool,
// }

// fn symbol_to_bytes(s: &str) -> [u8; 16] {
//     let mut buf = [0u8; 16];
//     let bytes = s.as_bytes();
//     let len = bytes.len().min(16);
//     buf[..len].copy_from_slice(&bytes[..len]);
//     buf
// }

fn main() {}
// fn main() {
//     dotenv::from_path("../.env").ok();
//     let private_key =
//         env::var("ADMIN_PHANTOM_PRIVATE_KEY").expect("ADMIN_PHANTOM_PRIVATE_KEY not set");
//     let payer = Rc::new(Keypair::from_base58_string(&private_key));

//     let client = Client::new(Cluster::Devnet, Rc::clone(&payer));
//     let program = client.program(perp::ID).unwrap();

//     let (global_config_pda, _) = Pubkey::find_program_address(&[b"global"], &perp::ID);

//     let markets_json = std::fs::read_to_string("../config/markets.json")
//         .expect("Failed to read config/markets.json");
//     let markets: Vec<MarketConfig> =
//         serde_json::from_str(&markets_json).expect("Failed to parse markets.json");

//     for mut market_cfg in markets {
//         let symbol = symbol_to_bytes(&market_cfg.symbol);
//         let (market_pda, _) =
//             Pubkey::find_program_address(&[b"market_seed", symbol.as_ref()], &perp::ID);

//         // Skip if already initialized on-chain
//         if program.rpc().get_account(&market_pda).is_ok() {
//             println!(
//                 "Market {} already initialized, skipping.",
//                 market_cfg.symbol
//             );
//             continue;
//         }

//         let oracle_pubkey: Pubkey = market_cfg
//             .oracle
//             .parse()
//             .unwrap_or_else(|_| panic!("Invalid oracle pubkey for {}", market_cfg.symbol));

//         let config = perp::types::SMParams {
//             max_leverage: market_cfg.max_leverage,
//             mmr_bps: market_cfg.maintenance_margin_bps,
//         };

//         let tx = program
//             .request()
//             .accounts(perp::client::accounts::InitializeMarket {
//                 payer: payer.pubkey(),
//                 global_config: global_config_pda,
//                 market: market_pda,
//                 oracle: oracle_pubkey,
//                 system_program: anchor_lang::system_program::ID,
//             })
//             .args(perp::client::args::InitializeMarket { symbol, config })
//             .send()
//             .unwrap_or_else(|e| panic!("Failed to initialize market {}: {e}", market_cfg.symbol));

//         println!("Initialized market {} | tx: {}", market_cfg.symbol, tx);

//         // TODO: Replace false with true and save in the markets.json
//         market_cfg.is_initialized = true;
//     }
// }
