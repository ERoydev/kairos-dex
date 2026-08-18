// Integration test for `open_position`, using LiteSVM with both `perp.so` and
// `liquidity_pool.so` loaded so the real credit CPI path executes end to end.
//
// Accounts a single open_position call needs, in setup order:
//   1. global_config        (perp::initialize_global)
//   2. lp_pool + lp_pool_usdc_vault + lp_mint (liquidity_pool::initialize_pool),
//      with pool.perp_program == market_vault so the CPI auth check in `credit` passes
//   3. market + market_vault (perp::initialize_market), which reads lp_pool's TVL
//      to seed its risk caps
//   4. usdc_mint, trader_usdc_ata (funded), fee_receiver_ata
//   5. a fabricated PriceUpdateV2 account owned by the pyth receiver program id —
//      we don't run the real Pyth receiver program on-chain, so this is hand-built
//   6. position PDA (init'd by the instruction itself)
//
// Build prerequisite (not run by `cargo test` automatically):
//   `cargo build-sbf --features dev` in programs/perp/programs/perp, so
//   target/deploy/perp.so exists for `common::perp_bytes()`. liquidity_pool.so is
//   pulled from the sibling liquidity-pool repo's own target/deploy — see
//   `common::liquidity_pool_bytes()`.
//
// STATUS: one passing happy-path test (long, under all caps). Not yet covered:
// short side, non-zero fee split (blocked by the seed LP deposit in `setup()`
// capping max_position_notional too low to produce fee >= 1), and
// failure cases (market paused, caps exceeded, stale price, unauthorized).

mod common;

use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_keypair::Keypair;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;

use perp::{
    state::{
        position::{Position, PositionType},
        syntetic_market::{SynteticMarket, TvlScaledCaps},
    },
    OpenPositionParams, SMParams,
};

use common::*;

// --- environment setup -------------------------------------------------------

struct Env {
    svm: LiteSVM,
    trader: Keypair,
    program_id: Pubkey,
    lp_program_id: Pubkey,
    global_config: Pubkey,
    market: Pubkey,
    market_vault: Pubkey,
    lp_pool: Pubkey,
    lp_pool_usdc_vault: Pubkey,
    usdc_mint: Pubkey,
    trader_usdc_ata: Pubkey,
    fee_receiver_ata: Pubkey,
}

/// Initializes a market with a small LP pool deposit, a funded trader, and a fee receiver.
fn setup() -> Env {
    let mut svm = LiteSVM::new();
    let program_id = perp::id();
    let lp_program_id = liquidity_pool::id();
    svm.add_program(program_id, perp_bytes()).unwrap();
    svm.add_program(lp_program_id, liquidity_pool_bytes())
        .unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();
    let trader = Keypair::new();
    svm.airdrop(&trader.pubkey(), 10 * LAMPORTS_PER_SOL)
        .unwrap();

    let usdc_mint = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(USDC_DECIMALS)
        .send()
        .unwrap();

    // global_config
    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();
    let init_global_ix =
        make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver, 10);
    send_ix(&mut svm, init_global_ix, &[&payer]).unwrap();

    // market + market_vault
    let sym = symbol("BTC-PERP");
    let market = market_pda(&program_id, &sym);
    let market_vault = market_vault_pda(&program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&program_id, &market);
    let oracle = Keypair::new().pubkey();

    // lp_pool — perp_program must be market_vault's address: that's the account
    // that actually signs the credit CPI (via invoke_signed on its own PDA seeds),
    // not the perp program id itself. See liquidity_pool::instructions::credit.
    // Initialized before the market since initialize_market now reads the pool's
    // TVL to seed the market's risk caps.
    let lp_pool = lp_pool_pda(&lp_program_id);
    let lp_pool_usdc_vault = lp_usdc_vault_pda(&lp_program_id);
    let lp_mint = lp_mint_pda(&lp_program_id);
    let init_pool_ix = make_initialize_pool_ix(
        lp_program_id,
        payer.pubkey(),
        lp_pool,
        usdc_mint,
        lp_pool_usdc_vault,
        lp_mint,
        market_vault,
    );
    send_ix(&mut svm, init_pool_ix, &[&payer]).unwrap();

    // Seed the pool with a small deposit so market caps (derived from lp_pool.total_assets
    // at initialize_market time) match the values the tests below were written against.
    seed_lp_pool(
        &mut svm,
        &payer,
        lp_program_id,
        usdc_mint,
        lp_pool,
        lp_pool_usdc_vault,
        lp_mint,
        50_000,
    );

    let init_market_ix = make_initialize_market_ix(
        program_id,
        payer.pubkey(),
        global_config,
        market,
        market_vault,
        insurance_fund_vault,
        lp_pool,
        oracle,
        usdc_mint,
        sym,
        SMParams {
            max_leverage: 20,
            mmr_bps: 500,
            feed_id: format!("0x{FEED_ID_HEX}"),
        },
    );
    send_ix(&mut svm, init_market_ix, &[&payer]).unwrap();

    // trader + fee receiver ATAs
    let trader_usdc_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&trader.pubkey())
        .send()
        .unwrap();
    MintTo::new(
        &mut svm,
        &payer,
        &usdc_mint,
        &trader_usdc_ata,
        10_000 * 1_000_000,
    )
    .send()
    .unwrap();

    let fee_receiver_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&fee_receiver)
        .send()
        .unwrap();

    Env {
        svm,
        trader,
        program_id,
        lp_program_id,
        global_config,
        market,
        market_vault,
        lp_pool,
        lp_pool_usdc_vault,
        usdc_mint,
        trader_usdc_ata,
        fee_receiver_ata,
    }
}

// --- tests -------------------------------------------------------------------

#[test]
fn test_open_position_ok() {
    let mut env = setup();
    let price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00 @ -8 exponent

    let position = position_pda(&env.program_id, &env.trader.pubkey(), &env.market);
    // Market caps are derived from initialize_market's hardcoded mock_lp_tvl = 50_000
    // (MicroUsdc), which makes max_position_notional a tiny 500 MicroUsdc — so these
    // amounts are deliberately small to stay under cap, not representative USDC sizes.
    let params = OpenPositionParams {
        leverage: 4,
        margin: 100,
        take_profit: 0,
        stop_loss: 0,
        position_type: PositionType::Long,
    };

    let trader_ata_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let market_vault_before = token_balance(&env.svm, &env.market_vault);
    let fee_receiver_before = token_balance(&env.svm, &env.fee_receiver_ata);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let lp_pool_assets_before = pool_total_assets(&env.svm, &env.lp_pool);

    let ix = make_open_position_ix(
        env.program_id,
        env.lp_program_id,
        env.trader.pubkey(),
        price_update,
        env.global_config,
        position,
        env.market_vault,
        env.market,
        env.lp_pool,
        env.lp_pool_usdc_vault,
        env.fee_receiver_ata,
        env.trader_usdc_ata,
        env.usdc_mint,
        params,
    );
    let res = send_ix(&mut env.svm, ix, &[&env.trader]);
    assert!(res.is_ok(), "open_position failed: {:?}", res.err());

    let account = env.svm.get_account(&position).expect("position not found");
    let data = Position::try_deserialize(&mut account.data.as_slice()).unwrap();

    assert_eq!(data.owner, env.trader.pubkey());
    assert_eq!(data.market, env.market);
    assert!(data.side == PositionType::Long);
    assert_eq!(data.notional, 400); // margin(100) * leverage(4), fee floors to 0 at this size
    assert_eq!(data.collateral, 100);
    assert_eq!(data.entry_price, 100 * 1_000_000); // $100.00 in MicroUsdc

    let market_account = env.svm.get_account(&env.market).unwrap();
    let market_data = SynteticMarket::try_deserialize(&mut market_account.data.as_slice()).unwrap();
    assert_eq!(market_data.oi_long, 400);
    assert_eq!(market_data.oi_short, 0);

    // Token movements. fee_to_pay is 0 at this notional (10bps of 400 floors to 0),
    // so trader only ever loses the margin itself, and the fee-split legs (protocol
    // + LP) are exercised as zero-amount transfers, not skipped — this does NOT
    // prove the fee-split arithmetic is correct for a non-zero fee, only that the
    // transfer wiring/authorities are right. See PR notes: max_position_notional is
    // capped at 500 by initialize_market's hardcoded mock_lp_tvl, so no amount under
    // that cap can produce fee >= 1 (needs notional >= 1000 for base_fee_bps=10).
    assert_eq!(
        trader_ata_before - token_balance(&env.svm, &env.trader_usdc_ata),
        100
    );
    assert_eq!(
        token_balance(&env.svm, &env.market_vault) - market_vault_before,
        100
    );
    assert_eq!(
        token_balance(&env.svm, &env.fee_receiver_ata),
        fee_receiver_before
    );
    assert_eq!(
        token_balance(&env.svm, &env.lp_pool_usdc_vault),
        lp_vault_before
    );
    assert_eq!(
        pool_total_assets(&env.svm, &env.lp_pool),
        lp_pool_assets_before
    );
}

/// Same setup, but with market caps widened so the trade actually produces a
/// non-zero fee, so we can verify the protocol/LP fee split actually lands in
/// the right accounts for the right amounts (not just that the transfers no-op).
#[test]
fn test_open_position_distributes_fees() {
    let mut env = setup();
    widen_market_caps(
        &mut env.svm,
        &env.market,
        TvlScaledCaps {
            max_position_notional: 1_000_000_000,
            max_user_notional: 1_000_000_000,
            max_oi_long: 1_000_000_000,
            max_oi_short: 1_000_000_000,
            max_skew: 20_000_000, // small relative to notional below, so skew_fee also kicks in
        },
    );
    let price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00 @ -8 exponent

    let position = position_pda(&env.program_id, &env.trader.pubkey(), &env.market);
    // margin=1_000_000 (1 USDC), leverage=10 -> notional_before_fee=10_000_000.
    // base_fee = 10bps of 10_000_000 = 10_000. Skew goes 0 -> 10_000_000 against a
    // 20_000_000 cap (worsens_skew=true), so skew_fee = 10_000_000/20_000_000 * 20bps
    // of notional = 10bps of 10_000_000 = 10_000. total fee = 20_000.
    // protocol_fees = 15% of 20_000 = 3_000, lp_fees = 17_000.
    let params = OpenPositionParams {
        leverage: 10,
        margin: 1_000_000,
        take_profit: 0,
        stop_loss: 0,
        position_type: PositionType::Long,
    };
    let margin = params.margin;
    let expected_fee = 20_000u64;
    let expected_protocol_fee = 3_000u64;
    let expected_lp_fee = 17_000u64;
    let expected_collateral = margin - expected_fee;

    let trader_ata_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let market_vault_before = token_balance(&env.svm, &env.market_vault);
    let fee_receiver_before = token_balance(&env.svm, &env.fee_receiver_ata);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let lp_pool_assets_before = pool_total_assets(&env.svm, &env.lp_pool);

    let ix = make_open_position_ix(
        env.program_id,
        env.lp_program_id,
        env.trader.pubkey(),
        price_update,
        env.global_config,
        position,
        env.market_vault,
        env.market,
        env.lp_pool,
        env.lp_pool_usdc_vault,
        env.fee_receiver_ata,
        env.trader_usdc_ata,
        env.usdc_mint,
        params,
    );
    let res = send_ix(&mut env.svm, ix, &[&env.trader]);
    assert!(res.is_ok(), "open_position failed: {:?}", res.err());

    let account = env.svm.get_account(&position).expect("position not found");
    let data = Position::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.owner, env.trader.pubkey());
    assert_eq!(data.market, env.market);
    assert!(data.side == PositionType::Long);
    assert_eq!(data.entry_price, 100 * 1_000_000);
    assert_eq!(data.collateral, expected_collateral);
    assert_eq!(data.notional, expected_collateral * 10);

    let market_account = env.svm.get_account(&env.market).unwrap();
    let market_data = SynteticMarket::try_deserialize(&mut market_account.data.as_slice()).unwrap();
    assert_eq!(market_data.oi_long, expected_collateral * 10);
    assert_eq!(market_data.oi_short, 0);

    // Trader's transfers cover collateral + both fee cuts: transfer_margin_to_vault
    // sends (margin + lp_fee) so the vault can forward lp_fee on to the pool via
    // credit_lp_pool while still being left holding exactly `margin`, and
    // transfer_protocol_fee sends protocol_fees directly. Total trader outflow is
    // therefore the full original margin.
    assert_eq!(
        trader_ata_before - token_balance(&env.svm, &env.trader_usdc_ata),
        margin
    );
    assert_eq!(
        token_balance(&env.svm, &env.market_vault) - market_vault_before,
        expected_collateral
    );
    assert_eq!(
        token_balance(&env.svm, &env.fee_receiver_ata) - fee_receiver_before,
        expected_protocol_fee
    );
    assert_eq!(
        token_balance(&env.svm, &env.lp_pool_usdc_vault) - lp_vault_before,
        expected_lp_fee
    );
    assert_eq!(
        pool_total_assets(&env.svm, &env.lp_pool) - lp_pool_assets_before,
        expected_lp_fee
    );
}
