use anchor_lang::prelude::*;

#[error_code]
pub enum PerpError {
    #[msg("Only the counter authority can update this counter")]
    Unauthorized,
    #[msg("Counter has reached the maximum value")]
    CounterOverflow,
    #[msg("Invalid Config for Market initialization")]
    InvalidConfig,
    #[msg("Authority cannot be the default pubkey")]
    InvalidAuthority,
    #[msg("Fee receiver cannot be the default pubkey")]
    InvalidFeeReceiver,
    #[msg("Math operation resulted in overflow or underflow")]
    MathOverflow,
    #[msg("Funding too early")]
    FundingTooEarly,
    #[msg("Market is already in the requested state")]
    MarketAlreadyInState,
    #[msg("Market is paused")]
    MarketPaused,
    #[msg("MaxSkewLimitExceeded")]
    MaxSkewLimitExceeded,
    #[msg("Position notional exceeds market's max position notional cap")]
    PositionExceedsMaxNotional,
    #[msg("Trade would exceed the market's open interest cap for this side")]
    OpenInterestCapExceeded,
    #[msg("Oracle price exponent did not match the expected scale")]
    UnexpectedOracleExponent,
    #[msg("Oracle returned a non-positive price")]
    InvalidOraclePrice,
    #[msg("Specified leverage is out of bounds")]
    LeverageOutOfBounds,
    #[msg("Cannot calculate Open Fee on Close Position")]
    CannotCalculateOpenFeeOnClosePosition,
}
