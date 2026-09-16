//! Dev-only: drops varied fake rows into the local sqlite db (see `make seed`).
//! Each market carries a different scenario (healthy/near-liquidation/imbalanced OI/inactive)
//! so funding + liquidation logic can be exercised against more than one shape of data.
use kairos_db::entities::{funding_updates, lp_events, position_events, positions};
use kairos_db::queries;
use migration::MigratorTrait;
use sea_orm::Set;

struct MarketSeed {
    market_pubkey: &'static str,
    symbol: &'static str,
    oracle: &'static str,
    oi_long: i64,
    oi_short: i64,
    interval_seconds: i32,
    is_active: bool,
    funding_rate_bps: i64,
    cumulative_funding_index: i64,

    position_pubkey: &'static str,
    owner: &'static str,
    side: &'static str,
    collateral: i64,
    notional: i64,
    entry_price: i64,

    pool_pubkey: &'static str,
    pool_usdc: i64,
    pool_shares: i64,
    provider: &'static str,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = kairos_db::create_pool(&database_url).await;
    migration::Migrator::up(&db, None)
        .await
        .expect("failed to run migrations");

    let markets = [
        // Healthy long, balanced OI, modest positive funding.
        MarketSeed {
            market_pubkey: "Market1111111111111111111111111111111111111",
            symbol: "SOL-PERP",
            oracle: "Oracle1111111111111111111111111111111111111",
            oi_long: 50_000,
            oi_short: 42_000,
            interval_seconds: 60,
            is_active: true,
            funding_rate_bps: 5,
            cumulative_funding_index: 42,
            position_pubkey: "Position111111111111111111111111111111111111",
            owner: "Owner11111111111111111111111111111111111111",
            side: "long",
            collateral: 1_000_000,
            notional: 10_000_000,
            entry_price: 150_00,
            pool_pubkey: "Pool11111111111111111111111111111111111111",
            pool_usdc: 5_000_000,
            pool_shares: 5_000_000,
            provider: "Provider111111111111111111111111111111111111",
        },
        // Short position near-liquidation: ~50x leverage, tight collateral cushion.
        MarketSeed {
            market_pubkey: "Market2222222222222222222222222222222222222",
            symbol: "BTC-PERP",
            oracle: "Oracle2222222222222222222222222222222222222",
            oi_long: 30_000,
            oi_short: 33_000,
            interval_seconds: 30,
            is_active: true,
            funding_rate_bps: -8,
            cumulative_funding_index: -12,
            position_pubkey: "Position222222222222222222222222222222222222",
            owner: "Owner22222222222222222222222222222222222222",
            side: "short",
            collateral: 200_000,
            notional: 10_000_000,
            entry_price: 65_000_00,
            pool_pubkey: "Pool22222222222222222222222222222222222222",
            pool_usdc: 8_000_000,
            pool_shares: 7_500_000,
            provider: "Provider222222222222222222222222222222222222",
        },
        // Long-heavy OI imbalance driving a large positive funding rate.
        MarketSeed {
            market_pubkey: "Market3333333333333333333333333333333333333",
            symbol: "ETH-PERP",
            oracle: "Oracle3333333333333333333333333333333333333",
            oi_long: 120_000,
            oi_short: 18_000,
            interval_seconds: 120,
            is_active: true,
            funding_rate_bps: 25,
            cumulative_funding_index: 210,
            position_pubkey: "Position333333333333333333333333333333333333",
            owner: "Owner33333333333333333333333333333333333333",
            side: "long",
            collateral: 2_500_000,
            notional: 12_000_000,
            entry_price: 3_200_00,
            pool_pubkey: "Pool33333333333333333333333333333333333333",
            pool_usdc: 6_000_000,
            pool_shares: 6_000_000,
            provider: "Provider333333333333333333333333333333333333",
        },
        // Inactive market — should be excluded by `find_active_markets`.
        MarketSeed {
            market_pubkey: "Market4444444444444444444444444444444444444",
            symbol: "DOGE-PERP",
            oracle: "Oracle4444444444444444444444444444444444444",
            oi_long: 5_000,
            oi_short: 5_200,
            interval_seconds: 60,
            is_active: false,
            funding_rate_bps: 0,
            cumulative_funding_index: 0,
            position_pubkey: "Position444444444444444444444444444444444444",
            owner: "Owner44444444444444444444444444444444444444",
            side: "long",
            collateral: 50_000,
            notional: 500_000,
            entry_price: 15,
            pool_pubkey: "Pool44444444444444444444444444444444444444",
            pool_usdc: 1_000_000,
            pool_shares: 1_000_000,
            provider: "Provider444444444444444444444444444444444444",
        },
    ];

    for (i, m) in markets.iter().enumerate() {
        let slot_base = (i as i64) * 10;

        queries::insert_market(
            &db,
            kairos_db::entities::markets::ActiveModel {
                market_pubkey: Set(m.market_pubkey.to_string()),
                symbol: Set(m.symbol.to_string()),
                oracle: Set(m.oracle.to_string()),
                oi_long: Set(m.oi_long),
                oi_short: Set(m.oi_short),
                interval_seconds: Set(m.interval_seconds),
                is_active: Set(m.is_active),
                ..Default::default()
            },
        )
        .await
        .ok();

        queries::upsert_position(
            &db,
            m.position_pubkey,
            positions::ActiveModel {
                position_pubkey: Set(m.position_pubkey.to_string()),
                owner: Set(m.owner.to_string()),
                market: Set(m.market_pubkey.to_string()),
                side: Set(m.side.to_string()),
                collateral: Set(m.collateral),
                notional: Set(m.notional),
                entry_price: Set(m.entry_price),
                entry_funding_index: Set(0),
                ..Default::default()
            },
        )
        .await
        .ok();

        queries::insert_position_event(
            &db,
            position_events::ActiveModel {
                event_type: Set("open".to_string()),
                position_pubkey: Set(m.position_pubkey.to_string()),
                owner: Set(m.owner.to_string()),
                market: Set(m.market_pubkey.to_string()),
                side: Set(m.side.to_string()),
                notional: Set(m.notional),
                price: Set(m.entry_price),
                tx_signature: Set(format!("SeedSig{}-open", i)),
                slot: Set(slot_base + 1),
                ..Default::default()
            },
        )
        .await
        .ok();

        queries::insert_funding_update(
            &db,
            funding_updates::ActiveModel {
                market: Set(m.market_pubkey.to_string()),
                funding_rate_bps: Set(m.funding_rate_bps),
                cumulative_funding_index: Set(m.cumulative_funding_index),
                oi_long: Set(m.oi_long),
                oi_short: Set(m.oi_short),
                tx_signature: Set(format!("SeedSig{}-funding", i)),
                slot: Set(slot_base + 2),
                ..Default::default()
            },
        )
        .await
        .ok();

        queries::upsert_lp_pool(&db, m.pool_pubkey, m.pool_usdc, m.pool_shares)
            .await
            .ok();

        queries::insert_lp_event(
            &db,
            lp_events::ActiveModel {
                event_type: Set("deposit".to_string()),
                provider: Set(m.provider.to_string()),
                usdc_amount: Set(m.pool_usdc),
                shares_amount: Set(m.pool_shares),
                tx_signature: Set(format!("SeedSig{}-lp", i)),
                slot: Set(slot_base + 3),
                ..Default::default()
            },
        )
        .await
        .ok();
    }

    println!(
        "seeded: {} markets (+position, position event, funding update, lp pool+event each)",
        markets.len()
    );
}
