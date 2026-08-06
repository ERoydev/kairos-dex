use anchor_lang::prelude::*;
use crate::{error::PerpError, events::MarketInitialized, state::global::GlobalConfig, syntetic_market::{FeeSchedule, FundingConfig, FundingFees, RiskManagementParameters, SynteticMarket, TvlScaledCaps}, utils::caps::compute_caps, GLOBAL_SEED, MARKET_SEED, MARKET_VERSION};

// Admin instruction
pub fn _initialize_market(ctx: Context<InitializeMarket>, symbol: [u8; 16], config: SMParams) -> Result<()> {
    require!(config.max_leverage > 0, PerpError::InvalidConfig);
    require!(config.mmr_bps < 10_000, PerpError::InvalidConfig);

    // TODO: Fix later
    let mock_lp_tvl = 50_000;

    let market = &mut ctx.accounts.market;
    market.version = MARKET_VERSION;
    market.bump = ctx.bumps.market;
    market.authority = ctx.accounts.payer.key();
    market.symbol = symbol;
    market.oracle = ctx.accounts.oracle.key();
    
    // runtime state
    market.oi_long = 0;
    market.oi_short = 0;
    market.funding_fees = FundingFees::default();

    // config
    market.risk_management = RiskManagementParameters {
        max_leverage: config.max_leverage,
        maintenance_margin_bps: config.mmr_bps,
        fee_schedule: FeeSchedule::default(),
        caps: compute_caps(mock_lp_tvl)
    };
    
    market.funding_config = FundingConfig::default();

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
        space = 8 + SynteticMarket::INIT_SPACE,
        seeds = [MARKET_SEED, symbol.as_ref()],
        bump
    )]
    pub market: Account<'info, SynteticMarket>,

    /// CHECK: It Is Checked
    #[account(constraint=oracle.key() != Pubkey::default())]
    pub oracle: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

// TODO: Maybe i need to adjust some formulas so for example open_long_interest and open_fee_bps can be adjusted dynamically on trader positions
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SMParams {
    pub max_leverage: u16,
    pub mmr_bps: u16,
}
