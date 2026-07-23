use anchor_lang::{
    prelude::Pubkey,
    solana_program::{instruction::Instruction, system_program},
    AccountDeserialize, InstructionData, ToAccountMetas,
};
use anchor_spl::token::TokenAccount;
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use liquidity_pool::{Pool, LP_MINT_SEED, USDC_VAULT_SEED};

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

fn make_deposit_ix(
    program_id: Pubkey,
    provider: Pubkey,
    pool: Pubkey,
    provider_ata: Pubkey,
    provider_lp_ata: Pubkey,
    usdc_mint: Pubkey,
    usdc_vault: Pubkey,
    lp_mint: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &liquidity_pool::instruction::Deposit { amount }.data(),
        liquidity_pool::accounts::Deposit {
            provider,
            pool,
            provider_ata,
            provider_lp_ata,
            usdc_mint,
            usdc_vault,
            lp_mint,
            token_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn get_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            owner.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
    .0
}

#[test]
fn test_deposit() {
    let mut svm = LiteSVM::new();
    let program_id = liquidity_pool::id();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/liquidity_pool.so"
    ));
    svm.add_program(program_id, bytes).unwrap();

    let (pool, _) = Pubkey::find_program_address(
        &[liquidity_pool::constants::LIQUIDITY_POOL_SEED],
        &program_id,
    );
    let usdc_mint = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(USDC_DECIMALS)
        .send()
        .unwrap();
    let (usdc_vault, _) = Pubkey::find_program_address(&[USDC_VAULT_SEED], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[LP_MINT_SEED], &program_id);

    // Initialize the pool.
    let blockhash = svm.latest_blockhash();
    let init_ix = make_initialize_pool_ix(program_id, pool, payer.pubkey(), usdc_mint, usdc_vault, lp_mint);
    let msg = Message::new_with_blockhash(&[init_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    // Create provider's USDC ATA and fund it.
    let deposit_amount = 1_000 * 1_000_000u64; // 1000 USDC
    let provider_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&payer.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut svm, &payer, &usdc_mint, &provider_ata, deposit_amount)
        .send()
        .unwrap();

    let provider_lp_ata = get_ata(&payer.pubkey(), &lp_mint);

    // Deposit.
    let blockhash = svm.latest_blockhash();
    let deposit_ix = make_deposit_ix(
        program_id,
        payer.pubkey(),
        pool,
        provider_ata,
        provider_lp_ata,
        usdc_mint,
        usdc_vault,
        lp_mint,
        deposit_amount,
    );
    let msg = Message::new_with_blockhash(&[deposit_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "deposit failed: {:?}", res.err());

    // Pool accounting updated.
    let pool_account = svm.get_account(&pool).unwrap();
    let pool_data = Pool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
    assert_eq!(pool_data.total_assets, deposit_amount);
    assert_eq!(pool_data.total_shares, deposit_amount); // 1:1 for first depositor

    // LP tokens minted to provider.
    let lp_ata_account = svm.get_account(&provider_lp_ata).unwrap();
    let lp_balance = TokenAccount::try_deserialize(&mut lp_ata_account.data.as_slice())
        .unwrap()
        .amount;
    assert_eq!(lp_balance, deposit_amount);

    // USDC moved from provider to vault.
    let usdc_ata_account = svm.get_account(&provider_ata).unwrap();
    let usdc_balance = TokenAccount::try_deserialize(&mut usdc_ata_account.data.as_slice())
        .unwrap()
        .amount;
    assert_eq!(usdc_balance, 0);
}
