// Integration test for `initialize_market`, using LiteSVM with both `perp.so` and
// `liquidity_pool.so` loaded, since initialize_market now reads the LP pool's
// on-chain TVL (lp_pool.total_assets) to seed the market's risk caps.
//
// Build prerequisite (not run by `cargo test` automatically):
//   `cargo build-sbf --features dev` in programs/perp/programs/perp, so
//   target/deploy/perp.so exists for `common::perp_bytes()`. liquidity_pool.so is
//   pulled from the sibling liquidity-pool repo's own target/deploy.

mod common;

use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use litesvm::LiteSVM;
use litesvm_token::CreateMint;
use solana_keypair::Keypair;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;

use perp::{
    state::syntetic_market::SynteticMarket, BASE_FEE_BPS, INTERVAL_SECONDS, MARKET_SEED,
    MARKET_VERSION, MAX_RATE_BPS, SENSITIVITY_BPS, SKEW_FEE_MAX_BPS,
};

use common::*;

// --- environment setup -------------------------------------------------------

/// Bundle of everything an `initialize_market` test needs. `usdc_mint` and `lp_pool`
/// are shared, symbol-independent state set up once here; `market` / `vault` /
/// `insurance_fund_vault` are per-test since each test picks its own symbol.
struct Env {
    svm: LiteSVM,
    payer: Keypair,
    program_id: Pubkey,
    global_config: Pubkey,
    lp_pool: Pubkey,
    usdc_mint: Pubkey,
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

    // initialize global config so market ix can read it
    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();
    let ix = make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver, 10);
    send_ix(&mut svm, ix, &[&payer]).unwrap();

    // lp_pool — initialize_market reads lp_pool.total_assets to seed the market's
    // risk caps, so the pool must exist before any initialize_market call. Its
    // `perp_program` field is irrelevant here since these tests never CPI into it.
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

    Env {
        svm,
        payer,
        program_id,
        global_config,
        lp_pool,
        usdc_mint,
    }
}

#[test]
fn test_initialize_market_ok() {
    let mut env = setup();
    let sym = symbol("BTC-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&env.program_id, &sym);
    let vault = market_vault_pda(&env.program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&env.program_id, &market);

    let ix = make_initialize_market_ix(
        env.program_id,
        env.payer.pubkey(),
        env.global_config,
        market,
        vault,
        insurance_fund_vault,
        env.lp_pool,
        oracle,
        env.usdc_mint,
        sym,
        default_config(),
    );
    assert!(send_ix(&mut env.svm, ix, &[&env.payer]).is_ok());

    let account = env.svm.get_account(&market).expect("market not found");
    let data = SynteticMarket::try_deserialize(&mut account.data.as_slice()).unwrap();

    let (_, expected_bump) =
        Pubkey::find_program_address(&[MARKET_SEED, sym.as_ref()], &env.program_id);

    assert_eq!(data.version, MARKET_VERSION);
    assert_eq!(data.bump, expected_bump);
    assert_eq!(data.authority, env.payer.pubkey());
    assert_eq!(data.symbol, sym);
    assert_eq!(data.oracle, oracle);
    assert_eq!(data.feed_id, default_config().feed_id);

    assert_eq!(data.oi_long, 0);
    assert_eq!(data.oi_short, 0);
    assert_eq!(data.funding_fees.cumulative_funding_index_bps, 0);
    assert_eq!(data.funding_fees.last_funding_time, 0);

    assert_eq!(data.risk_management.max_leverage, default_config().max_leverage);
    assert_eq!(data.risk_management.maintenance_margin_bps, default_config().mmr_bps);
    assert_eq!(data.risk_management.fee_schedule.base_fee_bps, BASE_FEE_BPS);
    assert_eq!(data.risk_management.fee_schedule.skew_fee_max_bps, SKEW_FEE_MAX_BPS);
    // setup()'s lp_pool has no deposits, so total_assets == 0 and every TVL-scaled cap is 0.
    assert_eq!(data.risk_management.caps.max_position_notional, 0);
    assert_eq!(data.risk_management.caps.max_user_notional, 0);
    assert_eq!(data.risk_management.caps.max_oi_long, 0);
    assert_eq!(data.risk_management.caps.max_oi_short, 0);
    assert_eq!(data.risk_management.caps.max_skew, 0);

    assert_eq!(data.funding_config.sensitivity_bps, SENSITIVITY_BPS);
    assert_eq!(data.funding_config.max_rate_bps, MAX_RATE_BPS);
    assert_eq!(data.funding_config.interval_seconds, INTERVAL_SECONDS);

    assert!(data.is_active);
}

#[test]
fn test_initialize_market_rejects_non_admin() {
    let mut env = setup();
    let attacker = Keypair::new();
    env.svm
        .airdrop(&attacker.pubkey(), 10 * LAMPORTS_PER_SOL)
        .unwrap();

    let sym = symbol("BTC-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&env.program_id, &sym);
    let vault = market_vault_pda(&env.program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&env.program_id, &market);

    let ix = make_initialize_market_ix(
        env.program_id,
        attacker.pubkey(),
        env.global_config,
        market,
        vault,
        insurance_fund_vault,
        env.lp_pool,
        oracle,
        env.usdc_mint,
        sym,
        default_config(),
    );
    assert!(send_ix(&mut env.svm, ix, &[&attacker]).is_err());
}

#[test]
fn test_initialize_market_rejects_zero_leverage() {
    let mut env = setup();
    let sym = symbol("ETH-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&env.program_id, &sym);
    let vault = market_vault_pda(&env.program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&env.program_id, &market);

    let mut config = default_config();
    config.max_leverage = 0;

    let ix = make_initialize_market_ix(
        env.program_id,
        env.payer.pubkey(),
        env.global_config,
        market,
        vault,
        insurance_fund_vault,
        env.lp_pool,
        oracle,
        env.usdc_mint,
        sym,
        config,
    );
    assert!(send_ix(&mut env.svm, ix, &[&env.payer]).is_err());
}

#[test]
fn test_initialize_market_rejects_default_oracle() {
    let mut env = setup();
    let sym = symbol("ARB-PERP");
    let market = market_pda(&env.program_id, &sym);
    let vault = market_vault_pda(&env.program_id, &market);
    let insurance_fund_vault = insurance_fund_vault_pda(&env.program_id, &market);

    let ix = make_initialize_market_ix(
        env.program_id,
        env.payer.pubkey(),
        env.global_config,
        market,
        vault,
        insurance_fund_vault,
        env.lp_pool,
        Pubkey::default(),
        env.usdc_mint,
        sym,
        default_config(),
    );
    assert!(send_ix(&mut env.svm, ix, &[&env.payer]).is_err());
}
