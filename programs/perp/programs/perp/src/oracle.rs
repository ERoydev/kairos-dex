use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::Price;

use crate::{alliases::MicroUsdc, error::PerpError, PYTH_PRICE_EXPONENT};

/// Converts a Pyth `Price` into `MicroUsdc` (6 decimals). Requires the feed's
/// exponent to match `PYTH_PRICE_EXPONENT`, since a differing exponent would
/// silently scale the price by the wrong power of ten.
pub fn convert_price_to_micro_usdc(price: &Price) -> Result<MicroUsdc> {
    require_eq!(
        price.exponent,
        PYTH_PRICE_EXPONENT,
        PerpError::UnexpectedOracleExponent
    );
    require!(price.price > 0, PerpError::InvalidOraclePrice);

    Ok((price.price / 100) as u64)
}

// TODO:
// Implement Oracle Guard
