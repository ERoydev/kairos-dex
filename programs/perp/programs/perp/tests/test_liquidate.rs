// Integration test for `liquidate`, using LiteSVM with both `perp.so` and
// `liquidity_pool.so` loaded (the LP pool is still needed to seed a market via
// `initialize_market`/`open_position`, even though `liquidate` itself never CPIs
// into it — the liquidation penalty is settled entirely between market_vault,
// the insurance fund, and the liquidator).
//
// Build prerequisite (not run by `cargo test` automatically):
//   `cargo build-sbf --features dev` in programs/perp/programs/perp, so
//   target/deploy/perp.so exists for `common::perp_bytes()`. liquidity_pool.so is
//   pulled from the sibling liquidity-pool repo's own target/deploy.
//
// Covers both branches of `_liquidate`:
//   - normal liquidation (equity > 0): penalty = equity, split 50/50 between the
//     liquidator and the insurance fund
//   - bad debt (equity <= 0): no equity left to split, so the insurance fund pays
//     the liquidator a small flat keeper reward instead
// plus the guard that a healthy position (equity > maintenance_margin) can't be
// liquidated at all.

mod common;

use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_keypair::Keypair;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;

use perp::{
    state::{
        position::PositionType,
        syntetic_market::{SynteticMarket, TvlScaledCaps},
    },
    OpenPositionParams, SMParams,
};

use common::*;

// --- environment setup -------------------------------------------------------

struct Env {
    svm: LiteSVM,
    trader: Keypair,
    liquidator: Keypair,
    program_id: Pubkey,
    lp_program_id: Pubkey,
    global_config: Pubkey,
    market: Pubkey,
    market_vault: Pubkey,
    insurance_fund_vault: Pubkey,
    lp_pool: Pubkey,
    lp_pool_usdc_vault: Pubkey,
    usdc_mint: Pubkey,
    trader_usdc_ata: Pubkey,
    liquidator_usdc_ata: Pubkey,
    fee_receiver_ata: Pubkey,
    payer: Keypair,
}

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
    let liquidator = Keypair::new();
    svm.airdrop(&liquidator.pubkey(), 10 * LAMPORTS_PER_SOL)
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

    // market + market_vault + insurance_fund_vault
    let sym = symbol("BTC-PERP");
    let market = market_pda(&program_id, &sym);
    let market_vault = market_vault_pda(&program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&program_id, &market);
    let oracle = Keypair::new().pubkey();

    // lp_pool must exist before initialize_market (it reads lp_pool.total_assets
    // to seed the market's risk caps), even though liquidate never touches it.
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

    // Widen caps immediately: every liquidate test opens a large-enough position
    // to actually breach maintenance margin, well past the default seed-tvl caps.
    widen_market_caps(
        &mut svm,
        &market,
        TvlScaledCaps {
            max_position_notional: 1_000_000_000,
            max_user_notional: 1_000_000_000,
            max_oi_long: 1_000_000_000,
            max_oi_short: 1_000_000_000,
            max_skew: 20_000_000,
        },
    );

    // trader + liquidator + fee receiver ATAs
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

    let liquidator_usdc_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&liquidator.pubkey())
        .send()
        .unwrap();

    let fee_receiver_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&fee_receiver)
        .send()
        .unwrap();

    Env {
        svm,
        trader,
        liquidator,
        program_id,
        lp_program_id,
        global_config,
        market,
        market_vault,
        insurance_fund_vault,
        lp_pool,
        lp_pool_usdc_vault,
        usdc_mint,
        trader_usdc_ata,
        liquidator_usdc_ata,
        fee_receiver_ata,
        payer,
    }
}

/// Opens a position for `env.trader` and returns its PDA.
fn open(env: &mut Env, price_update: Pubkey, params: OpenPositionParams) -> Pubkey {
    let position = position_pda(&env.program_id, &env.trader.pubkey(), &env.market);
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
    position
}

/// Funds the insurance fund vault directly with `amount` USDC (mint authority ==
/// `env.payer`, so this doesn't need a dedicated deposit instruction).
fn fund_insurance_fund(env: &mut Env, amount: u64) {
    MintTo::new(
        &mut env.svm,
        &env.payer,
        &env.usdc_mint,
        &env.insurance_fund_vault,
        amount,
    )
    .send()
    .unwrap();
}

// --- tests -------------------------------------------------------------------

/// margin=1_000_000, leverage=10 -> open fee 20_000, collateral 980_000,
/// notional 9_800_000 — same math as the close_position tests. Shared by every
/// test below so the numbers are easy to cross-check.
fn open_standard_long(env: &mut Env, price_update: Pubkey) -> Pubkey {
    open(
        env,
        price_update,
        OpenPositionParams {
            leverage: 10,
            margin: 1_000_000,
            take_profit: 0,
            stop_loss: 0,
            position_type: PositionType::Long,
        },
    )
}

/// A healthy position (small adverse move, well above maintenance margin) must
/// be rejected — liquidation isn't a way to force-close someone's profitable or
/// merely-slightly-losing position.
#[test]
fn test_liquidate_rejects_healthy_position() {
    let mut env = setup();
    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00
    let position = open_standard_long(&mut env, open_price_update);

    // -1% move: pnl = -98_000, equity = 980_000 - 98_000 = 882_000, maintenance
    // margin = 9_800_000 * 500bps = 490_000. equity(882_000) > margin(490_000),
    // so this position is nowhere near liquidatable.
    let bad_price_update = fabricate_price_update(&mut env.svm, 99_00_000_000); // $99.00
    let liquidate_ix = make_liquidate_ix(
        env.program_id,
        env.liquidator.pubkey(),
        env.trader.pubkey(),
        bad_price_update,
        position,
        env.market_vault,
        env.insurance_fund_vault,
        env.market,
        env.liquidator_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, liquidate_ix, &[&env.liquidator]);
    assert!(res.is_err(), "healthy position should not be liquidatable");

    // Position must still exist, untouched.
    let account = env.svm.get_account(&position).expect("position not found");
    assert!(account.lamports > 0);
}

/// -8% move leaves positive but sub-margin equity: normal liquidation branch,
/// penalty == equity, split 50/50 between liquidator and insurance fund.
#[test]
fn test_liquidate_normal_splits_penalty_50_50() {
    let mut env = setup();
    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00
    let position = open_standard_long(&mut env, open_price_update);

    // pnl = -8_000_000/100_000_000 * 9_800_000 = -784_000.
    // equity = 980_000 - 784_000 = 196_000. maintenance_margin = 490_000.
    // equity(196_000) <= margin(490_000) and > 0 -> normal liquidation.
    let liq_price_update = fabricate_price_update(&mut env.svm, 92_00_000_000); // $92.00

    let vault_before = token_balance(&env.svm, &env.market_vault);
    let insurance_before = token_balance(&env.svm, &env.insurance_fund_vault);
    let liquidator_before = token_balance(&env.svm, &env.liquidator_usdc_ata);

    let liquidate_ix = make_liquidate_ix(
        env.program_id,
        env.liquidator.pubkey(),
        env.trader.pubkey(),
        liq_price_update,
        position,
        env.market_vault,
        env.insurance_fund_vault,
        env.market,
        env.liquidator_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, liquidate_ix, &[&env.liquidator]);
    assert!(res.is_ok(), "liquidate failed: {:?}", res.err());

    let expected_penalty = 196_000u64;
    let expected_liquidator_cut = 98_000u64;
    let expected_insurance_cut = 98_000u64;

    assert_eq!(
        token_balance(&env.svm, &env.liquidator_usdc_ata) - liquidator_before,
        expected_liquidator_cut
    );
    assert_eq!(
        token_balance(&env.svm, &env.insurance_fund_vault) - insurance_before,
        expected_insurance_cut
    );
    assert_eq!(
        vault_before - token_balance(&env.svm, &env.market_vault),
        expected_penalty
    );

    let market_data = SynteticMarket::try_deserialize(
        &mut env.svm.get_account(&env.market).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(market_data.oi_long, 0);

    let closed = env
        .svm
        .get_account(&position)
        .map(|a| a.lamports)
        .unwrap_or(0);
    assert_eq!(closed, 0, "position account should be closed / drained");
}

/// A -15% move wipes out equity entirely (bad debt): no penalty to split, so the
/// insurance fund pays the liquidator a flat `BAD_DEBT_KEEPER_REWARD_BPS` reward
/// on notional instead, and nothing moves out of market_vault.
#[test]
fn test_liquidate_bad_debt_pays_keeper_from_insurance_fund() {
    let mut env = setup();
    fund_insurance_fund(&mut env, 1_000_000);

    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00
    let position = open_standard_long(&mut env, open_price_update);

    // pnl = -15_000_000/100_000_000 * 9_800_000 = -1_470_000.
    // equity = 980_000 - 1_470_000 = -490_000 <= 0 -> bad debt branch.
    let liq_price_update = fabricate_price_update(&mut env.svm, 85_00_000_000); // $85.00

    let vault_before = token_balance(&env.svm, &env.market_vault);
    let insurance_before = token_balance(&env.svm, &env.insurance_fund_vault);
    let liquidator_before = token_balance(&env.svm, &env.liquidator_usdc_ata);

    let liquidate_ix = make_liquidate_ix(
        env.program_id,
        env.liquidator.pubkey(),
        env.trader.pubkey(),
        liq_price_update,
        position,
        env.market_vault,
        env.insurance_fund_vault,
        env.market,
        env.liquidator_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, liquidate_ix, &[&env.liquidator]);
    assert!(res.is_ok(), "liquidate failed: {:?}", res.err());

    // reward = notional(9_800_000) * BAD_DEBT_KEEPER_REWARD_BPS(10) / 10_000 = 9_800.
    let expected_reward = 9_800u64;

    assert_eq!(
        token_balance(&env.svm, &env.liquidator_usdc_ata) - liquidator_before,
        expected_reward
    );
    assert_eq!(
        insurance_before - token_balance(&env.svm, &env.insurance_fund_vault),
        expected_reward
    );
    // Bad debt never touches market_vault — the trader's remaining collateral is
    // simply left behind there, not the liquidator's or insurance fund's problem.
    assert_eq!(token_balance(&env.svm, &env.market_vault), vault_before);

    let closed = env
        .svm
        .get_account(&position)
        .map(|a| a.lamports)
        .unwrap_or(0);
    assert_eq!(closed, 0, "position account should be closed / drained");
}

/// Mirrors the normal-split test but on the short side (price rises instead of
/// falls), to catch a sign error in `calculate_pnl`'s short branch and confirm
/// `oi_short` gets decremented on liquidation too.
#[test]
fn test_liquidate_short_position() {
    let mut env = setup();
    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00
    let position = open(
        &mut env,
        open_price_update,
        OpenPositionParams {
            leverage: 10,
            margin: 1_000_000,
            take_profit: 0,
            stop_loss: 0,
            position_type: PositionType::Short,
        },
    );

    // +8% move is a loss for a short: pnl = (100e6-108e6)*9_800_000/100e6 = -784_000.
    // Same equity/penalty numbers as the long normal-liquidation test.
    let liq_price_update = fabricate_price_update(&mut env.svm, 108_00_000_000); // $108.00

    let liquidator_before = token_balance(&env.svm, &env.liquidator_usdc_ata);
    let insurance_before = token_balance(&env.svm, &env.insurance_fund_vault);

    let liquidate_ix = make_liquidate_ix(
        env.program_id,
        env.liquidator.pubkey(),
        env.trader.pubkey(),
        liq_price_update,
        position,
        env.market_vault,
        env.insurance_fund_vault,
        env.market,
        env.liquidator_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, liquidate_ix, &[&env.liquidator]);
    assert!(res.is_ok(), "liquidate failed: {:?}", res.err());

    let expected_liquidator_cut = 98_000u64;
    let expected_insurance_cut = 98_000u64;

    assert_eq!(
        token_balance(&env.svm, &env.liquidator_usdc_ata) - liquidator_before,
        expected_liquidator_cut
    );
    assert_eq!(
        token_balance(&env.svm, &env.insurance_fund_vault) - insurance_before,
        expected_insurance_cut
    );

    let market_data = SynteticMarket::try_deserialize(
        &mut env.svm.get_account(&env.market).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(market_data.oi_short, 0);
}

/// Liquidation is permissionless: any keeper can trigger it, not just the market
/// authority. `setup()`'s liquidator is already an unrelated, unprivileged keypair,
/// so `test_liquidate_normal_splits_penalty_50_50` already exercises this — this
/// test just makes the guarantee explicit with a second, freshly-created keeper.
#[test]
fn test_liquidate_allows_any_keeper() {
    let mut env = setup();
    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00
    let position = open_standard_long(&mut env, open_price_update);
    let liq_price_update = fabricate_price_update(&mut env.svm, 92_00_000_000); // $92.00

    let random_keeper = Keypair::new();
    env.svm
        .airdrop(&random_keeper.pubkey(), 10 * LAMPORTS_PER_SOL)
        .unwrap();
    let keeper_usdc_ata = CreateAssociatedTokenAccount::new(&mut env.svm, &env.payer, &env.usdc_mint)
        .owner(&random_keeper.pubkey())
        .send()
        .unwrap();

    let liquidate_ix = make_liquidate_ix(
        env.program_id,
        random_keeper.pubkey(),
        env.trader.pubkey(),
        liq_price_update,
        position,
        env.market_vault,
        env.insurance_fund_vault,
        env.market,
        keeper_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, liquidate_ix, &[&random_keeper]);
    assert!(res.is_ok(), "liquidate failed: {:?}", res.err());
    assert_eq!(token_balance(&env.svm, &keeper_usdc_ata), 98_000);
}
