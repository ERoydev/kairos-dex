use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2};

use crate::{alliases::MicroUsdc, error::PerpError, PYTH_PRICE_EXPONENT};

pub type PriceAccount<'info> = Account<'info, PriceUpdateV2>;

pub struct OracleConfig {
    pub max_staleness_secs: u32,       // e.g. 60
    pub max_confidence_bps: u16,       // e.g. 100
    pub max_deviation_bps: u16,        // e.g. 200 (optional)
    // TODO: max_deviation is skipped for MVP, because it requires a second reference feed to compare against.
    // For MVP with single Pyth source: staleness + confidence is enough
}

impl Default for OracleConfig {
    fn default() -> Self {
        OracleConfig {
            max_staleness_secs: 60,     // reject if price older than 60s
            max_confidence_bps: 100,    // reject if confidence range > 1% of price
            max_deviation_bps: 200,     // reject if deviates > 2% from reference (skip for MVP)
        }
    }
}

// Oracle Guard used by every instruction that reads price
// open_position — entry price + guard checks.
// close_position — exit price.
// liquidate_position — mark price for MMR check.
// update_funding — mark price for skew calc (or skip if pure skew-driven).
pub struct OracleAdapter<'a, 'info> {
    // Stale prices — oracle hasn't updated in a while, real market has moved.
    // Manipulated prices — someone spoofs a bad tick.
    // Low-confidence prices — oracle itself signals it's uncertain.
    // Broken feeds — oracle down entirely.
    pub config: OracleConfig,
    pub price_account: &'a PriceAccount<'info>,
    pub feed_id: &'a [u8; 32],
}

impl<'a, 'info> OracleAdapter<'a, 'info> {
    pub fn new(price_account: &'a PriceAccount<'info>, feed_id: &'a [u8; 32]) -> OracleAdapter<'a, 'info> {
        let config = OracleConfig::default();
        OracleAdapter { config, price_account, feed_id }
    }

    pub fn read_price_guarded(&self, clock: &Clock) -> Result<MicroUsdc> {
        let price = self.price_account.get_price_no_older_than(
            clock,
            self.config.max_staleness_secs as u64,
            self.feed_id,
        )?;
        
        // Guard 1: staleness (already enforced above)
        // Guard 2: confidence
        let conf_bps = (price.conf as u128 * 10_000 / price.price as u128) as u64;
        require!(conf_bps <= self.config.max_confidence_bps as u64, PerpError::OracleUncertain);
        
        // Convert to MicroUSDC
        Ok(normalize_price_to_microusdc(&price.price, &price.exponent)?)
    }
}

/// Converts a Pyth `Price` into `MicroUsdc` (6 decimals). Requires the feed's
/// exponent to match `PYTH_PRICE_EXPONENT`, since a differing exponent would
/// silently scale the price by the wrong power of ten.
pub fn normalize_price_to_microusdc(price: &i64, price_exponent: &i32) -> Result<MicroUsdc> {
    require_eq!(
        *price_exponent,
        PYTH_PRICE_EXPONENT,
        PerpError::UnexpectedOracleExponent
    );
    require!(*price > 0, PerpError::InvalidOraclePrice);

    Ok((price / 100) as u64)
}
