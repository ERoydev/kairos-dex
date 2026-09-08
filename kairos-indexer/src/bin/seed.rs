//! Inserts mock rows into a dev database so the keeper bot has something to scan/act on
//! without needing a live devnet stream. Run `cargo db-fresh` first for a clean slate.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use solana_sdk::pubkey::Pubkey;

use kairos_indexer::db::{create_pool, entities::markets, entities::positions};
use kairos_indexer::Config;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config = Config::default();
    let db = create_pool(&config.database_url).await;

    // Idempotent: clear out whatever this script seeded last time before inserting again, so
    // `cargo db-seed` can be rerun freely without needing `cargo db-fresh` first.
    positions::Entity::delete_many()
        .exec(&db)
        .await
        .expect("failed to clear positions table");
    markets::Entity::delete_many()
        .exec(&db)
        .await
        .expect("failed to clear markets table");

    let now_dt = Utc::now();
    let now = now_dt.timestamp();

    // (symbol, interval_seconds, seconds_since_last_funding) — pick values that straddle the
    // due/not-due boundary so the keeper's funding-scan filter has something to exercise.
    let market_specs = [
        ("BTC-PERP", 3600, 7200), // overdue by 1h
        ("ETH-PERP", 3600, 60),   // not due yet
        ("SOL-PERP", 1800, 3600), // overdue by 30m
    ];

    let mut market_pubkeys = Vec::new();

    for (symbol, interval_seconds, age_seconds) in market_specs {
        let market_pubkey = Pubkey::new_unique().to_string();
        let oracle = Pubkey::new_unique().to_string();

        markets::ActiveModel {
            market_pubkey: Set(market_pubkey.clone()),
            symbol: Set(symbol.to_string()),
            oracle: Set(oracle),
            oi_long: Set(1_000_000),
            oi_short: Set(800_000),
            cumulative_funding_index: Set(0),
            last_funding_time: Set(now - age_seconds),
            interval_seconds: Set(interval_seconds),
            is_active: Set(true),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("failed to insert mock market");

        println!("seeded market {symbol} ({market_pubkey})");
        market_pubkeys.push((symbol, market_pubkey));
    }

    // A couple of open positions, one per side, on the first seeded market — enough to exercise
    // a liquidation-scan loop too.
    let (symbol, market_pubkey) = &market_pubkeys[0];
    for (side, entry_price) in [("long", 60_000_000_000i64), ("short", 60_500_000_000i64)] {
        let position_pubkey = Pubkey::new_unique().to_string();
        let owner = Pubkey::new_unique().to_string();

        positions::ActiveModel {
            position_pubkey: Set(position_pubkey.clone()),
            owner: Set(owner),
            market: Set(market_pubkey.clone()),
            side: Set(side.to_string()),
            collateral: Set(500_000_000),
            notional: Set(5_000_000_000),
            entry_price: Set(entry_price),
            entry_funding_index: Set(0),
            opened_at: Set(now_dt.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("failed to insert mock position");

        println!("seeded {side} position on {symbol} ({position_pubkey})");
    }

    println!("done");
}
