use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

use crate::{
    alliases::MicroUsdc,
    events::PositionOpened,
    global::GlobalConfig,
    position::{Position, PositionType},
    syntetic_market::SynteticMarket,
    utils::{caps, fee_model::FeeModel, skew::Skew},
    OracleAdapter, PerpError, GLOBAL_SEED, MARKET_VAULT, POSITION_SEED,
};

// This imports the credit function and Credit accounts struct from liquidity-pool
// #program macro auto-generates a cpi module gated behind feature `cpi`, containing wrapper fn per ix plus a matching cpi::accounts::* struct for each.
use liquidity_pool::Pool;
use solana_instructions_sysvar::ID as INSTRUCTIONS_SYSVAR_ID;

pub fn _open_position(ctx: Context<OpenPosition>, o_params: OpenPositionParams) -> Result<()> {
    require!(ctx.accounts.market.is_active, PerpError::MarketPaused);

    let market = &ctx.accounts.market;
    let is_long = o_params.position_type == PositionType::Long;
    let fee_schedule = &market.risk_management.fee_schedule;
    let trader = &ctx.accounts.trader;

    require!(
        market.risk_management.max_leverage >= o_params.leverage,
        PerpError::LeverageOutOfBounds
    );

    // Read oracle price
    let feed_id: [u8; 32] = get_feed_id_from_hex(&market.feed_id)?;
    let oracle_guard = OracleAdapter::new(&ctx.accounts.price_update, &feed_id);
    let asset_price: MicroUsdc = oracle_guard
        .read_price_guarded()
        .map_err(|_| PerpError::OracleGuardReadFailed)?;

    // Position_size
    let notional_before_fee: MicroUsdc = o_params
        .margin
        .checked_mul(o_params.leverage.into())
        .ok_or(PerpError::MathOverflow)?;

    // Caps are computed live off the shared pool's current TVL — never stored,
    // never stale. See `utils::caps`.
    let lp_tvl = ctx.accounts.lp_pool.total_assets;
    let max_skew = caps::max_skew(lp_tvl);

    // Skew (and the OI caps below) are checked against the *aggregate* across every
    // market, not this market's own oi_long/oi_short — they all draw on the same
    // shared pool, so a per-market check would let each market independently claim
    // the same capacity. See `GlobalConfig::total_oi_long/short`.
    let global_config = &ctx.accounts.global_config;
    let skew_model = Skew::new(
        &global_config.total_oi_long,
        &global_config.total_oi_short,
        &notional_before_fee,
        is_long,
    );
    let projected_skew = skew_model.projected_skew();
    // TODO: This require should be done after we got the final position_size after fees, but i leave it there until i refactor the code to avoid dublication
    require!(max_skew > projected_skew, PerpError::MaxSkewLimitExceeded);

    let worsens_skew = skew_model.worsens_skew();

    // calculate fee
    let fee_model = FeeModel::new(
        &notional_before_fee,
        fee_schedule,
        Some(&projected_skew),
        Some(&max_skew),
    );
    let fee_to_pay = fee_model.calculate_trade_fee(worsens_skew)?;

    // deduct fee from margin and get final values that we work with
    let margin = o_params
        .margin
        .checked_sub(fee_to_pay)
        .ok_or(PerpError::MathOverflow)?;
    let position_size: MicroUsdc = margin
        .checked_mul(o_params.leverage.into())
        .ok_or(PerpError::MathOverflow)?;

    // Cap: single position notional — a per-trade ceiling, no running total to
    // aggregate, so this one just needs the live TVL, nothing from global_config.
    require!(
        position_size <= caps::max_position_notional(lp_tvl),
        PerpError::PositionExceedsMaxNotional
    );

    // Cap: gross open interest for this side, aggregated across every market
    let (new_oi, oi_cap) = SynteticMarket::calc_open_interest(
        is_long,
        position_size,
        global_config.total_oi_long,
        global_config.total_oi_short,
        caps::max_oi_long(lp_tvl),
        caps::max_oi_short(lp_tvl),
    )?;
    require!(new_oi <= oi_cap, PerpError::OpenInterestCapExceeded);

    // TODO: max_user_notional isn't enforced here yet — there's no per-user
    // aggregate exposure tracked across a trader's positions in this market.

    // Distribute fees
    let (protocol_fees, lp_fees): (MicroUsdc, MicroUsdc) =
        fee_model.calc_distributed_fees(fee_to_pay)?;

    // Settle funds. credit_lp_pool's CPI can only pull from an account market_vault
    // itself controls (its `source` authority must be the signing `caller`), so
    // lp_fees has to round-trip through the vault rather than going straight from
    // the trader to the pool. That means the vault must receive margin + lp_fees
    // up front — depositing only `margin` here would leave the vault short by
    // lp_fees the moment credit_lp_pool forwards it out below, understating what's
    // actually backing `position.collateral` for every future payout.
    ctx.accounts
        .transfer_margin_to_vault(margin.checked_add(lp_fees).ok_or(PerpError::MathOverflow)?)?;
    ctx.accounts.transfer_protocol_fee(protocol_fees)?;
    ctx.accounts
        .credit_lp_pool(lp_fees, ctx.bumps.market_vault)?;

    let market_key = market.key();
    let entry_funding_index_bps = market.funding_fees.cumulative_funding_index_bps;

    // Create Position
    let position = &mut ctx.accounts.position;
    position.version = Position::get_version();
    position.owner = trader.key();
    position.market = market_key;
    position.side = o_params.position_type;
    position.collateral = margin;
    position.notional = position_size;
    position.entry_price = asset_price;
    position.opened_at = Clock::get()?.unix_timestamp;
    position.entry_funding_index_bps = entry_funding_index_bps;

    // Update this market's own OI (used for its funding rate) and the global
    // aggregate (used for pool-solvency caps) together, so they never drift apart.
    let market = &mut ctx.accounts.market;
    let global_config = &mut ctx.accounts.global_config;
    match o_params.position_type {
        PositionType::Long => {
            market.oi_long += position_size;
            global_config.total_oi_long += position_size;
        }
        PositionType::Short => {
            market.oi_short += position_size;
            global_config.total_oi_short += position_size;
        }
    }

    emit!(PositionOpened {
        market: market_key,
        position: position.key(),
        trader: trader.key(),
        oracle_price: asset_price,
        entry_funding_index_bps,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    // Account from Pyth to add into ix Context, when i need Price data.
    // This type will automatically perform a check for the account that is owned by Pyth Pull Oracle program
    //
    // Boxed: OpenPosition validates 13 accounts in one Anchor-generated try_accounts
    // function; left unboxed, several of the larger accounts here (this one, position,
    // market, lp_pool) together overflow the 4KB BPF stack frame during account
    // validation. Boxing moves the deserialized value to the heap.
    pub price_update: Box<Account<'info, PriceUpdateV2>>,

    // --- Perp accounts
    // `mut` + seeds-validated: this is now written to on every trade (aggregate OI
    // used by the pool-solvency caps), so it must be the canonical PDA, not just
    // any GlobalConfig-typed account the caller hands in.
    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = trader,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, trader.key().as_ref(), market.key().as_ref()],
        bump

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
    /// CHECK: lp pool prgoram ID stored for CPI access control
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
    pub system_program: Program<'info, System>,

    /// CHECK: address-constrained to the instructions sysvar; forwarded to liquidity-pool's credit CPI for caller introspection
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}

impl<'info> OpenPosition<'info> {
    /// Trader → market_vault. Escrows the trader's collateral for this position.
    pub fn transfer_margin_to_vault(&self, amount: MicroUsdc) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            self.token_program.key(),
            TransferChecked {
                from: self.trader_usdc_ata.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.market_vault.to_account_info(),
                authority: self.trader.to_account_info(),
            },
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }

    /// Trader → fee_receiver_ata. Protocol's cut of the trade fee.
    pub fn transfer_protocol_fee(&self, amount: MicroUsdc) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            self.token_program.key(),
            TransferChecked {
                from: self.trader_usdc_ata.to_account_info(),
                mint: self.usdc_mint.to_account_info(),
                to: self.fee_receiver_ata.to_account_info(),
                authority: self.trader.to_account_info(),
            },
        );
        transfer_checked(cpi_ctx, amount, self.usdc_mint.decimals)
    }

    /// market_vault → lp_pool, signed by the market_vault PDA. LP's cut of the trade
    /// fee, forwarded on to the pool via its `credit` instruction. Must run after
    /// `transfer_margin_to_vault`, since it draws from market_vault's own balance.
    pub fn credit_lp_pool(&self, amount: MicroUsdc, market_vault_bump: u8) -> Result<()> {
        let market_key = self.market.key();
        let vault_seeds: &[&[&[u8]]] =
            &[&[MARKET_VAULT, market_key.as_ref(), &[market_vault_bump]]];

        let cpi_accounts = liquidity_pool::cpi::accounts::Credit {
            caller: self.market_vault.to_account_info(),
            pool: self.lp_pool.to_account_info(),
            source: self.market_vault.to_account_info(),
            usdc_mint: self.usdc_mint.to_account_info(),
            usdc_vault: self.lp_pool_usdc_vault.to_account_info(),
            token_program: self.token_program.to_account_info(),
            instructions: self.instructions_sysvar.to_account_info(),
        };
        let cpi_ctx =
            CpiContext::new_with_signer(self.lp_pool_program.key(), cpi_accounts, vault_seeds);
        liquidity_pool::cpi::credit(cpi_ctx, amount)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenPositionParams {
    pub leverage: u16,
    pub margin: u64,
    pub take_profit: u64,
    pub stop_loss: u64,
    pub position_type: PositionType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyth_solana_receiver_sdk::price_update::{PriceFeedMessage, VerificationLevel};

    /// Hits the real Pyth Hermes network (https://hermes.pyth.network) for the live
    /// price of this market's feed_id, then runs it through the exact same
    /// `get_price_no_older_than` call used in `_open_position`.
    #[test]
    fn read_live_oracle_price() {
        let feed_id_hex = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
        let feed_id: [u8; 32] = get_feed_id_from_hex(feed_id_hex).unwrap();

        let url =
            format!("https://hermes.pyth.network/v2/updates/price/latest?ids[]={feed_id_hex}");
        let output = std::process::Command::new("curl")
            .args(["-s", "--max-time", "10", &url])
            .output()
            .expect("failed to run curl - is it installed and is there network access?");
        let body = String::from_utf8(output.stdout).expect("non-utf8 response from Hermes");

        // Pull the top-level "price": { ... } object out of the raw JSON response.
        let key = "\"price\":{";
        let idx = body
            .find(key)
            .unwrap_or_else(|| panic!("no price field in response: {body}"))
            + key.len();
        let inner = &body[idx..];
        let inner = &inner[..inner.find('}').unwrap()];

        let price: i64 = extract_field(inner, "price").parse().unwrap();
        let conf: u64 = extract_field(inner, "conf").parse().unwrap();
        let exponent: i32 = extract_field(inner, "expo").parse().unwrap();
        let publish_time: i64 = extract_field(inner, "publish_time").parse().unwrap();

        // Build a PriceUpdateV2 the way the Pyth Receiver program would, using the
        // real values we just fetched.
        let price_update = PriceUpdateV2 {
            write_authority: Pubkey::default(),
            verification_level: VerificationLevel::Full,
            price_message: PriceFeedMessage {
                feed_id,
                price,
                conf,
                exponent,
                publish_time,
                prev_publish_time: publish_time,
                ema_price: price,
                ema_conf: conf,
            },
            posted_slot: 0,
        };

        // Pretend "now" is right at publish_time so the 30s freshness check passes
        // regardless of how long ago Hermes actually published it.
        let clock = Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: publish_time,
        };

        let result = price_update
            .get_price_no_older_than(&clock, 30, &feed_id)
            .unwrap();

        let human_price = result.price as f64 * 10f64.powi(result.exponent);
        let micro_usdc: u64 = (result.price / 100) as u64; // exponent -8 -> 6 decimals

        println!("LIVE price from Pyth Hermes network:");
        println!(
            "raw: price={} conf={} exponent={} publish_time={}",
            result.price, result.conf, result.exponent, result.publish_time
        );
        println!("human price: ${human_price:.2}");
        println!("MicroUsdc: {micro_usdc}");
    }

    fn extract_field(json: &str, key: &str) -> String {
        let pat = format!("\"{key}\":");
        let start = json
            .find(&pat)
            .unwrap_or_else(|| panic!("field {key} not found in: {json}"))
            + pat.len();
        let rest = json[start..].trim_start();
        if let Some(stripped) = rest.strip_prefix('"') {
            stripped[..stripped.find('"').unwrap()].to_string()
        } else {
            let end = rest.find([',', '}']).unwrap_or(rest.len());
            rest[..end].trim().to_string()
        }
    }
}
