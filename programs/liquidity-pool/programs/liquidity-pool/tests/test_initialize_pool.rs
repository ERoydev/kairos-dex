
use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use liquidity_pool::{Pool, USDC_VAULT_SEED, LP_MINT_SEED};
use litesvm_token::CreateMint;
use solana_sdk::native_token::LAMPORTS_PER_SOL;

const USDC_DECIMALS: u8 = 6;

fn make_initialize_pool_ix(
    program_id: Pubkey,
    pool: Pubkey,
    payer: Pubkey,
    usdc_mint: Pubkey,
    usdc_vault: Pubkey,
    lp_mint: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &liquidity_pool::instruction::InitializePool {}.data(),
        liquidity_pool::accounts::InitializePool {
            pool,
            authority: payer,
            usdc_mint,
            usdc_vault,
            lp_mint,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

#[test]
fn test_initialize_pool() {
    let mut svm = LiteSVM::new();
    let program_id = liquidity_pool::id();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    let bytes = include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/liquidity_pool.so"));
    svm.add_program(program_id, bytes).unwrap();

    let (pool, _) = Pubkey::find_program_address(&[liquidity_pool::constants::LIQUIDITY_POOL_SEED], &program_id);
    let usdc_mint = CreateMint::new(&mut svm, &payer).authority(&payer.pubkey()).decimals(USDC_DECIMALS).send().unwrap();
    let (usdc_vault, _) = Pubkey::find_program_address(&[USDC_VAULT_SEED], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[LP_MINT_SEED], &program_id);

    let ix = make_initialize_pool_ix(program_id, pool, payer.pubkey(), usdc_mint, usdc_vault, lp_mint);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok());

    // Validate Pool account state
    let pool_account = svm.get_account(&pool).expect("pool account not found");
    let pool_data = Pool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();

    assert_eq!(pool_data.authority, payer.pubkey());
    assert_eq!(pool_data.usdc_mint, usdc_mint);
    assert_eq!(pool_data.usdc_vault, usdc_vault);
    assert_eq!(pool_data.version, 1);
    assert_eq!(pool_data.total_assets, 0);
    assert_eq!(pool_data.total_shares, 0);
}

#[test]
fn test_initialize_pool_already_exists() {
    let mut svm = LiteSVM::new();
    let program_id = liquidity_pool::id();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    let bytes = include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/liquidity_pool.so"));
    svm.add_program(program_id, bytes).unwrap();

    let (pool, _) = Pubkey::find_program_address(&[liquidity_pool::constants::LIQUIDITY_POOL_SEED], &program_id);
    let usdc_mint = CreateMint::new(&mut svm, &payer).authority(&payer.pubkey()).decimals(USDC_DECIMALS).send().unwrap();
    let (usdc_vault, _) = Pubkey::find_program_address(&[USDC_VAULT_SEED], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[LP_MINT_SEED], &program_id);

    let ix = make_initialize_pool_ix(program_id, pool, payer.pubkey(), usdc_mint, usdc_vault, lp_mint);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    let ix2 = make_initialize_pool_ix(program_id, pool, payer.pubkey(), usdc_mint, usdc_vault, lp_mint);
    let blockhash = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&payer]).unwrap();

    let res2 = svm.send_transaction(tx2);
    assert!(res2.is_err());
}
