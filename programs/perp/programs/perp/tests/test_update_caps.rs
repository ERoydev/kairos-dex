// Integration test for `update_caps`, the admin override for a market's
// TVL-scaled risk caps.
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

use perp::{state::syntetic_market::SynteticMarket, UpdateCapsParams};

use common::*;

struct Env {
    svm: LiteSVM,
    payer: Keypair,
    program_id: Pubkey,
    market: Pubkey,
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
        lp_pool,
        oracle,
        usdc_mint,
        sym,
        default_config(),
    );
    send_ix(&mut svm, init_market_ix, &[&payer]).unwrap();

    Env {
        svm,
        payer,
        program_id,
        market,
    }
}

#[test]
fn test_update_caps_ok() {
    let mut env = setup();

    let params = UpdateCapsParams {
        max_position_notional: Some(1_000_000),
        max_user_notional: Some(3_000_000),
        max_oi_long: Some(40_000_000),
        max_oi_short: Some(40_000_000),
        max_skew: Some(20_000_000),
    };
    let ix = make_update_caps_ix(
        env.program_id,
        env.payer.pubkey(),
        env.market,
        perp::instruction::UpdateCaps {
            params: params.clone(),
        },
    );
    assert!(send_ix(&mut env.svm, ix, &[&env.payer]).is_ok());

    let account = env.svm.get_account(&env.market).expect("market not found");
    let data = SynteticMarket::try_deserialize(&mut account.data.as_slice()).unwrap();

    assert_eq!(
        data.risk_management.caps.max_position_notional,
        params.max_position_notional.unwrap()
    );
    assert_eq!(
        data.risk_management.caps.max_user_notional,
        params.max_user_notional.unwrap()
    );
    assert_eq!(
        data.risk_management.caps.max_oi_long,
        params.max_oi_long.unwrap()
    );
    assert_eq!(
        data.risk_management.caps.max_oi_short,
        params.max_oi_short.unwrap()
    );
    assert_eq!(
        data.risk_management.caps.max_skew,
        params.max_skew.unwrap()
    );
}

#[test]
fn test_update_caps_partial_update_leaves_other_fields_untouched() {
    let mut env = setup();

    // setup()'s lp_pool has no deposits, so initialize_market seeded every cap at 0.
    let ix = make_update_caps_ix(
        env.program_id,
        env.payer.pubkey(),
        env.market,
        perp::instruction::UpdateCaps {
            params: UpdateCapsParams {
                max_position_notional: Some(500_000),
                max_user_notional: None,
                max_oi_long: None,
                max_oi_short: None,
                max_skew: None,
            },
        },
    );
    assert!(send_ix(&mut env.svm, ix, &[&env.payer]).is_ok());

    let account = env.svm.get_account(&env.market).expect("market not found");
    let data = SynteticMarket::try_deserialize(&mut account.data.as_slice()).unwrap();

    assert_eq!(data.risk_management.caps.max_position_notional, 500_000);
    assert_eq!(data.risk_management.caps.max_user_notional, 0);
    assert_eq!(data.risk_management.caps.max_oi_long, 0);
    assert_eq!(data.risk_management.caps.max_oi_short, 0);
    assert_eq!(data.risk_management.caps.max_skew, 0);
}

#[test]
fn test_update_caps_rejects_unauthorized() {
    let mut env = setup();
    let attacker = Keypair::new();
    env.svm
        .airdrop(&attacker.pubkey(), LAMPORTS_PER_SOL)
        .unwrap();

    let ix = make_update_caps_ix(
        env.program_id,
        attacker.pubkey(),
        env.market,
        perp::instruction::UpdateCaps {
            params: UpdateCapsParams {
                max_position_notional: Some(1),
                max_user_notional: None,
                max_oi_long: None,
                max_oi_short: None,
                max_skew: None,
            },
        },
    );
    assert!(send_ix(&mut env.svm, ix, &[&attacker]).is_err());
}
