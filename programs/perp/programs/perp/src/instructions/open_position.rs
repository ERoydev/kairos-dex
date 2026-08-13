use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, TransferChecked, transfer_checked};
use pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, get_feed_id_from_hex};

use crate::{alliases::MicroUsdc, events::PositionOpened, global::GlobalConfig, position::{Position, PositionType}, syntetic_market::SynteticMarket, utils::{fee_model::{calculate_trade_fee, worsens_skew}, projected_skew}, PerpError, MARKET_VAULT, POSITION_SEED};

// This imports the credit function and Credit accounts struct from liquidity-pool
// #program macro auto-generates a cpi module gated behind feature `cpi`, containing wrapper fn per ix plus a matching cpi::accounts::* struct for each.
use liquidity_pool::Pool;

pub fn _open_position(ctx: Context<OpenPosition>, o_params: OpenPositionParams) -> Result<()> {
    let market = &ctx.accounts.market;
    let price_update = &mut ctx.accounts.price_update;
    let is_long = o_params.position_type == PositionType::Long;
    let fee_schedule = &market.risk_management.fee_schedule;
    let trader = &ctx.accounts.trader;

    require!(market.is_active, PerpError::MarketPaused);
    
    // get_price_no_older_than will fail if the price update is more than 30 seconds old
    let maximum_age: u64 = 30;

    // Read oracle price
    let feed_id: [u8; 32] = get_feed_id_from_hex(&market.feed_id)?;
    let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?;
    let asset_price: MicroUsdc = (price.price / 100) as u64; // exponent -8 -> 6 decimals

    // Position_size 
    let notional_before_fee: MicroUsdc = o_params.margin.checked_mul(o_params.leverage).ok_or(PerpError::MathOverflow)?;

    // Observe imbalances that this trade can cause
    let projected_skew: MicroUsdc = projected_skew(&market.oi_long, &market.oi_short, &notional_before_fee, is_long);
    // TODO: This require should be done after we got the final position_size after fees, but i leave it there until i refactor the code to avoid dublication
    require!(market.risk_management.caps.max_skew > projected_skew, PerpError::MaxSkewLimitExceeded);

    let worsens_skew = worsens_skew(&market.oi_long, &market.oi_short, &notional_before_fee, is_long);

    // calculate fee
    let fee_to_pay: MicroUsdc = calculate_trade_fee(&notional_before_fee, fee_schedule, projected_skew, market.risk_management.caps.max_skew, worsens_skew);

    // deduct fee from margin and get final values that we work with
    let margin = o_params.margin.checked_sub(fee_to_pay).ok_or(PerpError::MathOverflow)?;
    let position_size: MicroUsdc = margin.checked_mul(o_params.leverage).ok_or(PerpError::MathOverflow)?;

    // Cap: single position notional
    require!(
        position_size <= market.risk_management.caps.max_position_notional,
        PerpError::PositionExceedsMaxNotional
    );

    // Cap: gross open interest for this side
    let (new_oi, oi_cap) = SynteticMarket::calc_open_interest(is_long, position_size, market.oi_long, market.oi_short, market.risk_management.caps.max_oi_long, market.risk_management.caps.max_oi_short)?;
    require!(new_oi <= oi_cap, PerpError::OpenInterestCapExceeded);

    // TODO: max_user_notional isn't enforced here yet — there's no per-user
    // aggregate exposure tracked across a trader's positions in this market.

    // Distribute fees
    let (protocol_fees, lp_fees): (MicroUsdc, MicroUsdc) = SynteticMarket::calc_distributed_fees(fee_to_pay)?;

    // Settle funds. Margin must land in market_vault before credit_lp_pool forwards
    // lp_fees back out of it — credit_lp_pool uses market_vault as its funding source.
    ctx.accounts.transfer_margin_to_vault(margin)?;
    ctx.accounts.transfer_protocol_fee(protocol_fees)?;
    ctx.accounts.credit_lp_pool(lp_fees, ctx.bumps.market_vault)?;

    let market_key = market.key();
    let entry_funding_index = market.funding_fees.cumulative_funding_index_bps;

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
    position.entry_funding_index = entry_funding_index;

    // Update Market open interest
    let market = &mut ctx.accounts.market;
    match o_params.position_type {
        PositionType::Long => market.oi_long += position_size,
        PositionType::Short => market.oi_short += position_size,
    }

    emit!(PositionOpened {
        market: market_key,
        position: position.key(),
        trader: trader.key(),
        oracle_price: asset_price,
        entry_funding_index,
    });

    Ok(())
}


#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    // Account from Pyth to add into ix Context, when i need Price data.
    // This type will automatically perform a check for the account that is owned by Pyth Pull Oracle program
    pub price_update: Account<'info, PriceUpdateV2>,

    // --- Perp accounts
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = trader,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, trader.key().as_ref(), market.key().as_ref()],
        bump

    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        seeds = [MARKET_VAULT, market.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = market_vault
    )]
    pub market_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub market: Account<'info, SynteticMarket>,

    // --- LP Pool accounts
    #[account(mut)]
    pub lp_pool: Account<'info, Pool>,
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
    pub fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,
    pub trader_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    // --- Mints
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    // --- System accounts
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
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
        let vault_seeds: &[&[&[u8]]] = &[&[MARKET_VAULT, market_key.as_ref(), &[market_vault_bump]]];

        let cpi_accounts = liquidity_pool::cpi::accounts::Credit {
            caller: self.market_vault.to_account_info(),
            pool: self.lp_pool.to_account_info(),
            source: self.market_vault.to_account_info(),
            usdc_mint: self.usdc_mint.to_account_info(),
            usdc_vault: self.lp_pool_usdc_vault.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            self.lp_pool_program.key(),
            cpi_accounts,
            vault_seeds,
        );
        liquidity_pool::cpi::credit(cpi_ctx, amount)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenPositionParams {
    pub leverage: u64,
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

        let url = format!(
            "https://hermes.pyth.network/v2/updates/price/latest?ids[]={feed_id_hex}"
        );
        let output = std::process::Command::new("curl")
            .args(["-s", "--max-time", "10", &url])
            .output()
            .expect("failed to run curl - is it installed and is there network access?");
        let body = String::from_utf8(output.stdout).expect("non-utf8 response from Hermes");

        // Pull the top-level "price": { ... } object out of the raw JSON response.
        let key = "\"price\":{";
        let idx = body.find(key).unwrap_or_else(|| panic!("no price field in response: {body}"))
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