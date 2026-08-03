use anchor_lang::prelude::*;
use crate::{MARKET_SEED, MARKET_VERSION, GLOBAL_SEED, events::MarketInitialized, market::Market, error::PerpError, state::global::GlobalConfig};

// Admin instruction
pub fn _initialize_market(ctx: Context<InitializeMarket>, symbol: [u8; 16], config: MConfig) -> Result<()> {
    require!(config.max_leverage > 0, PerpError::InvalidConfig);
    require!(config.max_open_interest > 0, PerpError::InvalidConfig);
    require!(config.open_fee_bps < 10_000, PerpError::InvalidConfig);
    require!(config.close_fee_bps < 10_000, PerpError::InvalidConfig);
    require!(config.mmr_bps < 10_000, PerpError::InvalidConfig);

    let market = &mut ctx.accounts.market;
    market.version = MARKET_VERSION;
    market.bump = ctx.bumps.market;
    market.authority = ctx.accounts.payer.key();
    market.symbol = symbol;
    market.oracle = ctx.accounts.oracle.key();
    market.max_leverage = config.max_leverage;
    market.max_open_interest = config.max_open_interest;
    market.open_interest_long = config.open_interest_long;
    market.open_interest_short = config.open_interest_short;

    market.maintenance_margin_bps = config.mmr_bps;
    market.open_fee_bps = config.open_fee_bps;
    market.close_fee_bps = config.close_fee_bps;

    market.is_active = true;

    emit!(MarketInitialized {
        market: ctx.accounts.market.key(),
        symbol,
        authority: ctx.accounts.payer.key(),
    });


    Ok(())
}

#[derive(Accounts)]
#[instruction(symbol: [u8; 16])] 
pub struct InitializeMarket<'info> {
    #[account(
        mut,
        // ANCHOR: Only Global Admin can call this instruction
        constraint = payer.key() == global_config.authority @ PerpError::Unauthorized 
    )]
    pub payer: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = payer,
        space = 8 + Market::INIT_SPACE,
        seeds = [MARKET_SEED, symbol.as_ref()],
        bump
    )]
    pub market: Account<'info, Market>,

    /// CHECK: It Is Checked
    #[account(constraint=oracle.key() != Pubkey::default())]
    pub oracle: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MConfig {
    pub max_leverage: u64,
    pub max_open_interest: u64,

    pub open_interest_long: u64,
    pub open_interest_short: u64,

    pub mmr_bps: u64,

    pub open_fee_bps: u64,
    pub close_fee_bps: u64,
}
