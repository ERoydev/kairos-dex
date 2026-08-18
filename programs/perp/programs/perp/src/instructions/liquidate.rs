use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    alliases::MicroUsdc, events::PositionLiquidated, oracle::convert_price_to_micro_usdc, position::{Position, PositionType}, syntetic_market::SynteticMarket, utils::pnl::{apply_funding, calculate_pnl}, PerpError, BAD_DEBT_KEEPER_REWARD_BPS, INSURANCE_FUND_VAULT, MARKET_VAULT, POSITION_SEED, PYTH_MAX_PRICE_AGE_SECONDS
};

// Anyone can call this (permissionless keeper). No trade fee is charged here —
// the maintenance margin buffer is what's supposed to cover the liquidator's cost.
// As an incentive, the position account's rent goes to the liquidator instead of the trader.
pub fn _liquidate(ctx: Context<Liquidate>) -> Result<()> {
    let market = &ctx.accounts.market;
    let position = &ctx.accounts.position;

    let feed_id: [u8; 32] = get_feed_id_from_hex(&market.feed_id)?;
    let price = ctx.accounts.price_update.get_price_no_older_than(
        &Clock::get()?,
        PYTH_MAX_PRICE_AGE_SECONDS,
        &feed_id,
    )?;
    let exit_price: MicroUsdc = convert_price_to_micro_usdc(&price)?;

    // Settle funding accrued since the position was opened
    let collateral = apply_funding(
        position.side,
        position.collateral,
        position.notional,
        position.entry_funding_index_bps,
        market.funding_fees.cumulative_funding_index_bps,
    );

    let pnl = calculate_pnl(
        position.side,
        position.entry_price,
        exit_price,
        position.notional,
    );

    // Maintenance margin requirement = notional * maintenance_margin_bps / 10_000.
    let maintenance_margin: MicroUsdc = (position.notional as u128)
        .checked_mul(market.risk_management.maintenance_margin_bps as u128)
        .and_then(|v| v.checked_div(10_000))
        .ok_or(PerpError::MathOverflow)? as MicroUsdc;

    // equity = collateral (after funding) + pnl, can go negative once underwater
    let equity = collateral as i64 + pnl;
    require!(
        equity <= maintenance_margin as i64,
        PerpError::PositionNotLiquidatable
    );

    let market_vault_bump = ctx.bumps.market_vault;
    let insurance_fund_bump = ctx.bumps.insurance_fund_vault;

    let liquidator_reward: MicroUsdc;

    if equity > 0 {
        // Normal liquidation: the whole surviving equity IS the penalty, split 50/50.
        let penalty = equity as u64;
        let liquidator_cut = penalty / 2;
        let insurance_cut = penalty - liquidator_cut;

        liquidator_reward = liquidator_cut;

        if liquidator_cut > 0 {
            ctx.accounts
                .transfer_to_liquidator(liquidator_cut, market_vault_bump)?;
        }
        if insurance_cut > 0 {
            ctx.accounts
                .transfer_to_insurance_fund(insurance_cut, market_vault_bump)?;
        }
    } else {
        // Bad debt: nothing left to penalize, so just pay the keeper a small flat
        // reward from the insurance fund for doing the cleanup.
        let reward = (position.notional as u128)
            .checked_mul(BAD_DEBT_KEEPER_REWARD_BPS as u128)
            .and_then(|v| v.checked_div(10_000))
            .ok_or(PerpError::MathOverflow)? as u64;
        liquidator_reward = reward;

        if reward > 0 {
            ctx.accounts
                .transfer_insurance_to_liquidator(reward, insurance_fund_bump)?;
        }
    }

    let market_key = market.key();
    let side = position.side;
    let notional = position.notional;
    let position_key = position.key();
    let trader_key = ctx.accounts.trader.key();
    let liquidator_key = ctx.accounts.liquidator.key();
    let market = &mut ctx.accounts.market;
    
    match side {
        PositionType::Long => market.oi_long -= notional,
        PositionType::Short => market.oi_short -= notional,
    }

    emit!(PositionLiquidated {
        market: market_key,
        position: position_key,
        trader: trader_key,
        liquidator: liquidator_key,
        exit_price,
        pnl,
        equity,
        liquidator_reward,
        bad_debt: equity <= 0,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub liquidator: Signer<'info>,

    /// CHECK: only used to derive/validate the position PDA and receive payout via its ATA
    pub trader: UncheckedAccount<'info>,

    pub price_update: Box<Account<'info, PriceUpdateV2>>,

    // Seeds bind this account to `trader` and `market`, so a mismatched trader or
    // market simply fails PDA derivation — no extra ownership check needed.
    #[account(
        mut,
        close = liquidator, // rent goes to the liquidator as their incentive
        seeds = [POSITION_SEED, trader.key().as_ref(), market.key().as_ref()],
        bump,
    )]
    pub position: Box<Account<'info, Position>>,

    #[account(
        mut,
        seeds = [MARKET_VAULT, market.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = market_vault
    )]
    pub market_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [INSURANCE_FUND_VAULT, market.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = insurance_fund_vault
    )]
    pub insurance_fund_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub market: Box<Account<'info, SynteticMarket>>,

    // --- Ata's
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = liquidator,
    )]
    pub liquidator_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    // --- Mints
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    // --- System accounts
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> Liquidate<'info> {
    fn vault_signer_seeds<'a>(market_key: &'a Pubkey, bump: &'a [u8; 1]) -> [&'a [u8]; 3] {
        [MARKET_VAULT, market_key.as_ref(), bump]
    }

    fn insurance_fund_signer_seeds<'a>(market_key: &'a Pubkey, bump: &'a [u8; 1]) -> [&'a [u8]; 3] {
        [INSURANCE_FUND_VAULT, market_key.as_ref(), bump]
    }

    /// market_vault → liquidator_usdc_ata, signed by the market_vault PDA. The keeper's
    /// 50% cut of the equity that survives the liquidation.
    pub fn transfer_to_liquidator(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [market_vault_bump];
        let seeds: &[&[&[u8]]] = &[&Self::vault_signer_seeds(&market_key, &bump)];

        let cpi_ctx = CpiContext::new_with_signer(
            self.token_program.key(),
            TransferChecked {
                from: self.market_vault.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.liquidator_usdc_ata.to_account_info(),
                authority: self.market_vault.to_account_info(),
            },
            seeds,
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }

    /// market_vault → insurance_fund_vault, signed by the market_vault PDA. The insurance
    /// fund's 50% cut of the liquidation penalty.
    pub fn transfer_to_insurance_fund(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [market_vault_bump];
        let seeds: &[&[&[u8]]] = &[&Self::vault_signer_seeds(&market_key, &bump)];

        let cpi_ctx = CpiContext::new_with_signer(
            self.token_program.key(),
            TransferChecked {
                from: self.market_vault.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.insurance_fund_vault.to_account_info(),
                authority: self.market_vault.to_account_info(),
            },
            seeds,
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }

    /// insurance_fund_vault → liquidator_usdc_ata, signed by the insurance_fund_vault PDA.
    /// Small flat keeper reward on a bad-debt liquidation, where there's no equity left to split.
    pub fn transfer_insurance_to_liquidator(&self, amount: MicroUsdc, insurance_fund_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [insurance_fund_bump];
        let seeds: &[&[&[u8]]] = &[&Self::insurance_fund_signer_seeds(&market_key, &bump)];

        let cpi_ctx = CpiContext::new_with_signer(
            self.token_program.key(),
            TransferChecked {
                from: self.insurance_fund_vault.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.liquidator_usdc_ata.to_account_info(),
                authority: self.insurance_fund_vault.to_account_info(),
            },
            seeds,
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }
}
