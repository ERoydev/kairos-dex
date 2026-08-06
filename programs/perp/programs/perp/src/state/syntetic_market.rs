use anchor_lang::prelude::*;

use crate::{
    alliases::MicroUsdc, BASE_FEE_BPS, INTERVAL_SECONDS, MAX_RATE_BPS, SENSITIVITY_BPS,
    SKEW_FEE_MAX_BPS,
};

/*
Market PDA

To handle which markets my protocol supports their risk config(max leverage, OI caps, fees)

Basically is going to be handled by On-chain instruction to create the Market setup
and an off-chain script that reads from .json and calls initialize_market for each entry.
It is not a service, it's just a dev script something that is admin-triggered event manually.
*/

#[account]
#[derive(InitSpace)]
pub struct SynteticMarket {
    pub version: u8,
    pub bump: u8,
    pub authority: Pubkey, // admin who can update config
    pub symbol: [u8; 16],  // e.g. "BTC-PERP", padded, each char stored in ASCII number

    pub oracle: Pubkey, // Pyth/Switchboard price feed account

    // runtime state
    pub oi_long: MicroUsdc,  // current total long size
    pub oi_short: MicroUsdc, // current total short size
    pub funding_fees: FundingFees,

    // config
    pub risk_management: RiskManagementParameters,
    pub funding_config: FundingConfig,

    pub is_active: bool, // admin can pause a market
    pub _reserved: [u8; 64],
}

/// Layer 1 of Risk management
#[derive(Clone, InitSpace, AnchorSerialize, AnchorDeserialize)]
pub struct RiskManagementParameters {
    pub max_leverage: u16,
    pub maintenance_margin_bps: u16, // MMR, liquidation threshold
    pub fee_schedule: FeeSchedule,   // base fees + skew curve
    pub caps: TvlScaledCaps,
}

#[derive(Clone, InitSpace, AnchorSerialize, AnchorDeserialize)]
/// Those get updated, when TVL in the pool grows or shrinks
pub struct TvlScaledCaps {
    pub max_position_notional: MicroUsdc, // single position size cap -> 100,000 for example means, a Position cannot exceed this in USDC
    pub max_user_notional: MicroUsdc, // cap on the sum of all one user's positions in that market.
    pub max_oi_long: MicroUsdc,       // gross open interest caps per side
    pub max_oi_short: MicroUsdc,
    pub max_skew: MicroUsdc, // net directional cap, 20% of Total Value Locked in LP Pool
}

#[derive(Clone, InitSpace, AnchorSerialize, AnchorDeserialize)]
pub struct FeeSchedule {
    /// Flat fee applied to every trade, in basis points (1 bps = 0.01%)
    pub base_fee_bps: u16,

    /// Max extra fee added when a trade pushes skew to the cap, in bps
    pub skew_fee_max_bps: u16,
}

impl Default for FeeSchedule {
    fn default() -> Self {
        FeeSchedule {
            base_fee_bps: BASE_FEE_BPS,
            skew_fee_max_bps: SKEW_FEE_MAX_BPS,
        }
    }
}

#[derive(Clone, InitSpace, AnchorSerialize, AnchorDeserialize, Default)]
pub struct FundingFees {
    // The bot every hour calls update_funding to update cumulative_index
    pub cumulative_funding_index_bps: i64,
    pub last_funding_time: i64,
}

#[derive(Clone, InitSpace, AnchorSerialize, AnchorDeserialize)]
pub struct FundingConfig {
    pub sensitivity_bps: u16,
    pub max_rate_bps: u16,
    pub interval_seconds: u16,
}

impl Default for FundingConfig {
    fn default() -> Self {
        FundingConfig {
            sensitivity_bps: SENSITIVITY_BPS,
            max_rate_bps: MAX_RATE_BPS,
            interval_seconds: INTERVAL_SECONDS,
        }
    }
}

// GLOBAL
// exposure_multiplier — multiplier on LP backing for global net delta cap.
// max_oracle_staleness — max oracle age.
// max_oracle_deviation_bps — max divergence from reference feed.
// insurance_fund_min_balance — floor that gates ADL.
