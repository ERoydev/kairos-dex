// Integration test for `update_funding`, the permissionless keeper instruction
// that advances a market's cumulative funding index based on its long/short
// open-interest skew.
//
// Build prerequisite (not run by `cargo test` automatically):
//   `cargo build-sbf --features dev` in programs/perp/programs/perp, so
//   target/deploy/perp.so exists for `common::perp_bytes()`. liquidity_pool.so is
//   pulled from the sibling liquidity-pool repo's own target/deploy.
//
// The pure math (`compute_skew_ratio_bps` / `compute_funding_rate`) already has
// unit tests next to the instruction. This file covers what those can't: the
// `interval_seconds` time gate, state mutation through a real instruction
// dispatch, sign handling for short-heavy skew, accumulation across repeated
// calls, and that the instruction is callable by any signer.

mod common;

use anchor_lang::prelude::Pubkey;
use anchor_lang::AccountDeserialize;
use litesvm::LiteSVM;
use litesvm_token::CreateMint;
use solana_keypair::Keypair;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;

use perp::state::syntetic_market::SynteticMarket;

use common::*;

struct Env {
    svm: LiteSVM,
    payer: Keypair,
    program_id: Pubkey,
    market: Pubkey,
    lp_pool: Pubkey,
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

    let usdc_mint = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(USDC_DECIMALS)
        .send()
        .unwrap();

    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();
    let ix = make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver, 10);
    send_ix(&mut svm, ix, &[&payer]).unwrap();

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
        program_id,
    );
    send_ix(&mut svm, init_pool_ix, &[&payer]).unwrap();

    let sym = symbol("BTC-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&program_id, &sym);
    let vault = market_vault_pda(&program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&program_id, &market);

    let init_market_ix = make_initialize_market_ix(
        program_id,
        payer.pubkey(),
        global_config,
        market,
        vault,
        insurance_fund_vault,
        oracle,
        usdc_mint,
        sym,
        default_config(),
    );
    send_ix(&mut svm, init_market_ix, &[&payer]).unwrap();

    // update_funding's skew_ratio_bps needs a nonzero max_skew, which is derived
    // live from lp_pool.total_assets; a target of 2_500 gives max_skew = 500
    // (20% of TVL), matching what tests below were written against.
    widen_lp_pool_tvl(
        &mut svm,
        &payer,
        lp_program_id,
        usdc_mint,
        lp_pool,
        lp_pool_usdc_vault,
        lp_mint,
        2_500,
    );

    Env {
        svm,
        payer,
        program_id,
        market,
        lp_pool,
    }
}

fn market_state(env: &Env) -> SynteticMarket {
    SynteticMarket::try_deserialize(&mut env.svm.get_account(&env.market).unwrap().data.as_slice())
        .unwrap()
}

/// `FundingFees` starts at `last_funding_time = 0` and LiteSVM's clock also
/// starts at `unix_timestamp = 0`, so calling immediately after `initialize_market`
/// (elapsed = 0) must be rejected by the `interval_seconds` (3600s) gate.
#[test]
fn test_update_funding_rejects_too_early() {
    let mut env = setup();

    let ix = make_update_funding_ix(env.program_id, env.payer.pubkey(), env.market, env.lp_pool);
    let res = send_ix(&mut env.svm, ix, &[&env.payer]);
    assert!(res.is_err(), "funding update before interval should fail");

    let market = market_state(&env);
    assert_eq!(market.funding_fees.cumulative_funding_index_bps, 0);
    assert_eq!(market.funding_fees.last_funding_time, 0);
}

/// Long-heavy skew (oi_long=1_250_000, oi_short=500_000, max_skew=500 -> a huge
/// skew_ratio_bps) drives funding_rate_bps to its positive clamp: with the
/// default config (sensitivity_bps=75, max_rate_bps=75), any skew_ratio_bps
/// beyond 10_000 saturates at +75. Also checks last_funding_time advances to
/// the warped clock value.
#[test]
fn test_update_funding_computes_and_clamps_positive_rate() {
    let mut env = setup();
    set_market_oi(&mut env.svm, &env.market, 1_250_000, 500_000);
    warp_clock_to(&mut env.svm, 3_600);

    let ix = make_update_funding_ix(env.program_id, env.payer.pubkey(), env.market, env.lp_pool);
    let res = send_ix(&mut env.svm, ix, &[&env.payer]);
    assert!(res.is_ok(), "update_funding failed: {:?}", res.err());

    let market = market_state(&env);
    assert_eq!(market.funding_fees.cumulative_funding_index_bps, 75);
    assert_eq!(market.funding_fees.last_funding_time, 3_600);
}

/// Mirrors the positive-clamp test but short-heavy, to catch a sign error in
/// `compute_skew_ratio_bps`/`compute_funding_rate` — funding_rate_bps should
/// clamp at -75, not +75 or panic on the negative intermediate values.
#[test]
fn test_update_funding_computes_negative_rate_for_short_skew() {
    let mut env = setup();
    set_market_oi(&mut env.svm, &env.market, 500_000, 1_250_000);
    warp_clock_to(&mut env.svm, 3_600);

    let ix = make_update_funding_ix(env.program_id, env.payer.pubkey(), env.market, env.lp_pool);
    let res = send_ix(&mut env.svm, ix, &[&env.payer]);
    assert!(res.is_ok(), "update_funding failed: {:?}", res.err());

    let market = market_state(&env);
    assert_eq!(market.funding_fees.cumulative_funding_index_bps, -75);
}

/// `cumulative_funding_index_bps` is an accumulator, not an overwrite: two
/// successive calls a full interval apart should each add +75, landing on 150.
#[test]
fn test_update_funding_accumulates_across_calls() {
    let mut env = setup();
    set_market_oi(&mut env.svm, &env.market, 1_250_000, 500_000);

    warp_clock_to(&mut env.svm, 3_600);
    let ix = make_update_funding_ix(env.program_id, env.payer.pubkey(), env.market, env.lp_pool);
    send_ix(&mut env.svm, ix, &[&env.payer]).unwrap();

    warp_clock_to(&mut env.svm, 7_200);
    // Same instruction + accounts as the first call: without a fresh blockhash
    // this produces an identical signature, which LiteSVM rejects as a replay.
    env.svm.expire_blockhash();
    let ix = make_update_funding_ix(env.program_id, env.payer.pubkey(), env.market, env.lp_pool);
    let res = send_ix(&mut env.svm, ix, &[&env.payer]);
    assert!(res.is_ok(), "second update_funding failed: {:?}", res.err());

    let market = market_state(&env);
    assert_eq!(market.funding_fees.cumulative_funding_index_bps, 150);
    assert_eq!(market.funding_fees.last_funding_time, 7_200);
}

/// `update_funding` is explicitly permissionless (invoked by any `perp-keeper`
/// bot), so an unrelated, unprivileged keypair must be able to call it — not
/// just the market authority.
#[test]
fn test_update_funding_allows_any_signer() {
    let mut env = setup();
    set_market_oi(&mut env.svm, &env.market, 1_250_000, 500_000);
    warp_clock_to(&mut env.svm, 3_600);

    let random_keeper = Keypair::new();
    env.svm
        .airdrop(&random_keeper.pubkey(), LAMPORTS_PER_SOL)
        .unwrap();

    let ix = make_update_funding_ix(env.program_id, random_keeper.pubkey(), env.market, env.lp_pool);
    let res = send_ix(&mut env.svm, ix, &[&random_keeper]);
    assert!(res.is_ok(), "update_funding failed: {:?}", res.err());

    let market = market_state(&env);
    assert_eq!(market.funding_fees.cumulative_funding_index_bps, 75);
}
