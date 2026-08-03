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
}
