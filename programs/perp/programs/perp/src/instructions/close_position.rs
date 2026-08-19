use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    alliases::MicroUsdc,
    events::PositionClosed,
    global::GlobalConfig,
    position::{Position, PositionType},
    syntetic_market::SynteticMarket,
    utils::{
        fee_model::FeeModel,
        pnl::{apply_funding, calculate_pnl, settle},
    },
    OracleAdapter, PerpError, MARKET_VAULT, POSITION_SEED,
};

use liquidity_pool::Pool;

// Closing is allowed even while the market is paused (`is_active = false`) —
// pause only blocks new positions, it shouldn't trap traders already in one.
// Closing position handles `withdraw` functionality, so Win or Loss are handled here.
pub fn _close_position(ctx: Context<ClosePosition>) -> Result<()> {
    let market = &ctx.accounts.market;
    let position = &ctx.accounts.position;

    let feed_id: [u8; 32] = get_feed_id_from_hex(&market.feed_id)?;
    let oracle_guard = OracleAdapter::new(&ctx.accounts.price_update, &feed_id);
    let exit_price: MicroUsdc = oracle_guard
        .read_price_guarded(&Clock::get()?)
        .map_err(|_| PerpError::OracleGuardReadFailed)?;

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

    // Close fee, same flat schedule charged on open. Capped at collateral so we
    // never try to move more out of the vault than this position actually owns.
    let fee_model = FeeModel::new(
        &position.notional,
        &market.risk_management.fee_schedule,
        None,
        None,
    );
    // TODO: This is wrong i risk to lose fee if the funding have eaten most of the collateral, so protocol can collect little or no close fee on that position.
    // Here i just have protection against underflow
    let close_fee = fee_model.calculate_base_fee(None)?.min(collateral);
    let collateral_after_fee = collateral - close_fee;

    // (amount_to_pay_trader, credit_amount_to_pool, debit_amount_from_pool)
    let (payout, credit_to_pool, debit_from_pool) = settle(collateral_after_fee, pnl, 0);
    let (protocol_fees, lp_fees): (MicroUsdc, MicroUsdc) =
        fee_model.calc_distributed_fees(close_fee)?;

    let market_vault_bump = ctx.bumps.market_vault;

    // Peel the close fee out of the vault first, same 15/85 split as on open.
    ctx.accounts
        .transfer_protocol_fee(protocol_fees, market_vault_bump)?;
    ctx.accounts.credit_lp_pool(lp_fees, market_vault_bump)?;

    // Then settle the trade itself between trader, vault and pool.
    // `payout` is the trader's *total* due; when it's pool-funded (debit_from_pool > 0)
    // that slice comes straight from the pool below, so the vault only owes the rest.
    let vault_payout = payout - debit_from_pool;
    if vault_payout > 0 {
        ctx.accounts
            .transfer_to_trader(vault_payout, market_vault_bump)?;
    }
    if credit_to_pool > 0 {
        ctx.accounts
            .credit_lp_pool(credit_to_pool, market_vault_bump)?;
    }
    if debit_from_pool > 0 {
        ctx.accounts
            .debit_lp_pool(debit_from_pool, market_vault_bump)?;
    }

    let market_key = market.key();
    let side = position.side;
    let notional = position.notional;
    let position_key = position.key();
    let trader_key = ctx.accounts.trader.key();

    let market = &mut ctx.accounts.market;
    match side {
        PositionType::Long => market.oi_long -= notional,
        PositionType::Short => market.oi_short -= notional,
    }

    emit!(PositionClosed {
        market: market_key,
        position: position_key,
        trader: trader_key,
        exit_price,
        pnl,
        payout,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    pub price_update: Box<Account<'info, PriceUpdateV2>>,

    // --- Perp accounts
    pub global_config: Account<'info, GlobalConfig>,

    // Seeds already bind this account to `trader` and `market`, so a mismatched
    // trader or market simply fails PDA derivation — no extra ownership check needed.
    #[account(
        mut,
        close = trader, // ANCHOR: deletes the account, all lamports `rent` are transferred to trader
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

    #[account(mut)]
    pub market: Box<Account<'info, SynteticMarket>>,

    // --- LP Pool accounts
    #[account(mut)]
    pub lp_pool: Box<Account<'info, Pool>>,
    #[account(mut)]
    pub lp_pool_usdc_vault: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: lp pool program ID stored for CPI access control
    pub lp_pool_program: UncheckedAccount<'info>,

    // --- Ata's
    /// The account responsible to store and collect protocol fees profit
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = global_config.fee_receiver,
    )]
    pub fee_receiver_ata: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub trader_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    // --- Mints
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    // --- System accounts
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> ClosePosition<'info> {
    fn vault_signer_seeds<'a>(market_key: &'a Pubkey, bump: &'a [u8; 1]) -> [&'a [u8]; 3] {
        [MARKET_VAULT, market_key.as_ref(), bump]
    }

    /// market_vault → trader_usdc_ata, signed by the market_vault PDA. Pays out the
    /// trader's remaining collateral plus any profit already covered by the vault.
    pub fn transfer_to_trader(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [market_vault_bump];
        let seeds: &[&[&[u8]]] = &[&Self::vault_signer_seeds(&market_key, &bump)];

        let cpi_ctx = CpiContext::new_with_signer(
            self.token_program.key(),
            TransferChecked {
                from: self.market_vault.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.trader_usdc_ata.to_account_info(),
                authority: self.market_vault.to_account_info(),
            },
            seeds,
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }

    /// market_vault → fee_receiver_ata, signed by the market_vault PDA. Protocol's cut of the close fee.
    pub fn transfer_protocol_fee(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [market_vault_bump];
        let seeds: &[&[&[u8]]] = &[&Self::vault_signer_seeds(&market_key, &bump)];

        let cpi_ctx = CpiContext::new_with_signer(
            self.token_program.key(),
            TransferChecked {
                from: self.market_vault.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.fee_receiver_ata.to_account_info(),
                authority: self.market_vault.to_account_info(),
            },
            seeds,
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }

    /// market_vault → lp_pool, signed by the market_vault PDA. Used for the LP's cut
    /// of the close fee, and separately for crediting a trader's net loss to the pool.
    pub fn credit_lp_pool(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [market_vault_bump];
        let vault_seeds: &[&[&[u8]]] = &[&Self::vault_signer_seeds(&market_key, &bump)];

        let cpi_accounts = liquidity_pool::cpi::accounts::Credit {
            caller: self.market_vault.to_account_info(),
            pool: self.lp_pool.to_account_info(),
            source: self.market_vault.to_account_info(),
            usdc_mint: self.usdc_mint.to_account_info(),
            usdc_vault: self.lp_pool_usdc_vault.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };
        let cpi_ctx =
            CpiContext::new_with_signer(self.lp_pool_program.key(), cpi_accounts, vault_seeds);
        liquidity_pool::cpi::credit(cpi_ctx, amount)
    }

    /// lp_pool → trader_usdc_ata directly, signed by the market_vault PDA (the account
    /// the pool recognizes as its perp counterpart). Pays profit beyond the trader's
    /// own collateral, so it doesn't need to round-trip through market_vault.
    pub fn debit_lp_pool(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let bump = [market_vault_bump];
        let vault_seeds: &[&[&[u8]]] = &[&Self::vault_signer_seeds(&market_key, &bump)];

        let cpi_accounts = liquidity_pool::cpi::accounts::Debit {
            caller: self.market_vault.to_account_info(),
            pool: self.lp_pool.to_account_info(),
            usdc_vault: self.lp_pool_usdc_vault.to_account_info(),
            destination: self.trader_usdc_ata.to_account_info(),
            usdc_mint: self.usdc_mint.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };
        let cpi_ctx =
            CpiContext::new_with_signer(self.lp_pool_program.key(), cpi_accounts, vault_seeds);
        liquidity_pool::cpi::debit(cpi_ctx, amount)
    }
}
