use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;

use perp::state::global::GlobalConfig;

mod common;
use common::*;

fn setup() -> (LiteSVM, Keypair, Pubkey) {
    let mut svm = LiteSVM::new();
    let program_id = perp::id();
    svm.add_program(program_id, perp_bytes()).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    (svm, payer, program_id)
}

#[test]
fn test_initialize_global_ok() {
    let (mut svm, payer, program_id) = setup();
    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();

    let ix = make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver, 10);
    assert!(send_ix(&mut svm, ix, &[&payer]).is_ok());

    let account = svm
        .get_account(&global_config)
        .expect("global_config not found");
    let data = GlobalConfig::try_deserialize(&mut account.data.as_slice()).unwrap();

    assert_eq!(data.authority, payer.pubkey());
    assert_eq!(data.fee_receiver, fee_receiver);
    assert_eq!(data.max_markets, 10);
    assert!(!data.is_paused);
}

#[test]
fn test_initialize_global_rejects_default_fee_receiver() {
    let (mut svm, payer, program_id) = setup();
    let global_config = global_config_pda(&program_id);

    let ix = make_initialize_global_ix(
        program_id,
        payer.pubkey(),
        global_config,
        Pubkey::default(),
        10,
    );
    assert!(send_ix(&mut svm, ix, &[&payer]).is_err());
}

#[test]
fn test_update_global_pauses() {
    let (mut svm, payer, program_id) = setup();
    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();

    let init_ix =
        make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver, 10);
    send_ix(&mut svm, init_ix, &[&payer]).unwrap();

    let update_ix = make_update_global_ix(
        program_id,
        payer.pubkey(),
        global_config,
        perp::instruction::UpdateGlobal {
            params: perp::UpdateGlobalParams {
                authority: None,
                fee_receiver: None,
                is_paused: Some(true),
                max_markets: None,
            },
        },
    );
    assert!(send_ix(&mut svm, update_ix, &[&payer]).is_ok());

    let account = svm.get_account(&global_config).unwrap();
    let data = GlobalConfig::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert!(data.is_paused);
}

#[test]
fn test_update_global_rejects_unauthorized() {
    let (mut svm, payer, program_id) = setup();
    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();

    let init_ix =
        make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver, 10);
    send_ix(&mut svm, init_ix, &[&payer]).unwrap();

    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let update_ix = make_update_global_ix(
        program_id,
        attacker.pubkey(),
        global_config,
        perp::instruction::UpdateGlobal {
            params: perp::UpdateGlobalParams {
                authority: None,
                fee_receiver: None,
                is_paused: Some(true),
                max_markets: None,
            },
        },
    );
    assert!(send_ix(&mut svm, update_ix, &[&attacker]).is_err());
}
