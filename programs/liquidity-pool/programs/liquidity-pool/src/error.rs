use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Only the counter authority can update this counter")]
    Unauthorized,
    #[msg("Counter has reached the maximum value")]
    CounterOverflow,
    #[msg("Invalid pool version")]
    InvalidPoolVersion,
    #[msg("Pool has no shares outstanding")]
    ZeroShares,
    #[msg("Pool has insufficient funds to cover payout")]
    InsufficientFunds,
}
