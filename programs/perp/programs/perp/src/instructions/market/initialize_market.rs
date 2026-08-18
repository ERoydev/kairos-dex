use crate::{
    error::PerpError, events::MarketInitialized, state::global::GlobalConfig, syntetic_market::{
        FeeSchedule, FundingConfig, FundingFees, RiskManagementParameters, SynteticMarket,
    }, utils::caps::compute_caps, GLOBAL_SEED, INSURANCE_FUND_VAULT, MARKET_SEED, MARKET_VAULT, MARKET_VERSION
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use liquidity_pool::{Pool, LIQUIDITY_POOL_SEED};

// Admin instruction
pub fn _initialize_market(
    ctx: Context<InitializeMarket>,
    symbol: [u8; 16],
    config: SMParams,
) -> Result<()> {
    require!(config.max_leverage > 0, PerpError::InvalidConfig);
    require!(config.mmr_bps < 10_000, PerpError::InvalidConfig);

    let lp_tvl = ctx.accounts.lp_pool.total_assets;

    let market = &mut ctx.accounts.market;
    market.version = MARKET_VERSION;
    market.bump = ctx.bumps.market;
    market.authority = ctx.accounts.payer.key();
    market.symbol = symbol;

    // Set oracle and feed_id
    market.oracle = ctx.accounts.oracle.key();
    market.feed_id = config.feed_id;

    // runtime state
    market.oi_long = 0;
    market.oi_short = 0;
    market.funding_fees = FundingFees::default();

    // config
    market.risk_management = RiskManagementParameters {
        max_leverage: config.max_leverage,
        maintenance_margin_bps: config.mmr_bps,
        fee_schedule: FeeSchedule::default(),
        caps: compute_caps(lp_tvl),
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

    /// Sets all the GlobalConfig for that market
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

    /// The market Vault, used for temporary storage of funds
    #[account(
        init,
        payer = payer,
        seeds = [MARKET_VAULT, market.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = vault
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    /// Insurance fund used for liquidation
    #[account(
        init,
        payer = payer,
        seeds = [INSURANCE_FUND_VAULT, market.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = insurance_fund_vault
    )]
    pub insurance_fund_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// LP pool TVL is used to seed this market's risk caps
    #[account(
        seeds = [LIQUIDITY_POOL_SEED],
        bump = lp_pool.bump,
        seeds::program = liquidity_pool::ID,
    )]
    pub lp_pool: Box<Account<'info, Pool>>,

    /// CHECK: It Is Checked
    #[account(constraint=oracle.key() != Pubkey::default())]
    pub oracle: UncheckedAccount<'info>,

    pub usdc_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SMParams {
    pub max_leverage: u16,
    pub mmr_bps: u16,
    pub feed_id: String,
}
