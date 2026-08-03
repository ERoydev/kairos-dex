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
    perp_program: Pubkey,
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
            perp_program,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn make_credit_ix(
    program_id: Pubkey,
    caller: Pubkey,
    pool: Pubkey,
    source: Pubkey,
    usdc_mint: Pubkey,
    usdc_vault: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &liquidity_pool::instruction::Credit { amount }.data(),
        liquidity_pool::accounts::Credit {
            caller,
            pool,
            source,
            usdc_mint,
            usdc_vault,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    )
}

// Spins up LiteSVM, loads the program, creates USDC mint, derives PDAs, and initializes the pool.
// perp_program is the pubkey that will be stored in pool.perp_program —
// only this key is allowed to call credit/debit.
// In production this would be the perp program's PDA; in tests we use a keypair we control.
fn setup(perp_program: Pubkey) -> (LiteSVM, Keypair, Pubkey, Pubkey, Pubkey, Pubkey) {
    let mut svm = LiteSVM::new();
    let program_id = liquidity_pool::id();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/liquidity_pool.so"
    ));
    svm.add_program(program_id, bytes).unwrap();

    let usdc_mint = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(USDC_DECIMALS)
        .send()
        .unwrap();
    let (usdc_vault, _) = Pubkey::find_program_address(&[USDC_VAULT_SEED], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[LP_MINT_SEED], &program_id);
    let (pool, _) = Pubkey::find_program_address(
        &[liquidity_pool::constants::LIQUIDITY_POOL_SEED],
        &program_id,
    );

    let init_ix = make_initialize_pool_ix(
        program_id,
        pool,
        payer.pubkey(),
        usdc_mint,
        usdc_vault,
        lp_mint,
        perp_program,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[init_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    (svm, payer, pool, usdc_mint, usdc_vault, lp_mint)
}

// Happy path: perp keypair (acting as the perp program) pushes USDC into the pool.
// In production this is called when a trader loses — the perp program sends their loss to the pool.
// We simulate the perp program by using a keypair whose pubkey == pool.perp_program.
#[test]
fn test_credit() {
    // perp_keypair simulates the perp program. Its pubkey is registered in pool.perp_program at init,
    // so it is the only caller authorized to credit/debit.
    let perp_keypair = Keypair::new();
    let (mut svm, payer, pool, usdc_mint, usdc_vault, _lp_mint) = setup(perp_keypair.pubkey());
    let program_id = liquidity_pool::id();

    svm.airdrop(&perp_keypair.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let credit_amount = 500 * 1_000_000u64; // 500 USDC

    // Create a USDC token account owned by the perp keypair and fund it.
    // In production, the perp program holds the trader's collateral before settlement.
    let perp_usdc_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&perp_keypair.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut svm, &payer, &usdc_mint, &perp_usdc_ata, credit_amount)
        .send()
        .unwrap();

    let blockhash = svm.latest_blockhash();
    let credit_ix = make_credit_ix(
        program_id,
        perp_keypair.pubkey(),
        pool,
        perp_usdc_ata,
        usdc_mint,
        usdc_vault,
        credit_amount,
    );
    let msg = Message::new_with_blockhash(&[credit_ix], Some(&payer.pubkey()), &blockhash);
    // Both payer (fee payer) and perp_keypair (caller/signer) must sign.
    // In production, perp_keypair is replaced by the perp program signing via invoke_signed.
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &perp_keypair])
            .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "credit failed: {:?}", res.err());

    let pool_account = svm.get_account(&pool).unwrap();
    let pool_data = Pool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
    assert_eq!(pool_data.total_assets, credit_amount);

    let vault_account = svm.get_account(&usdc_vault).unwrap();
    let vault_balance = TokenAccount::try_deserialize(&mut vault_account.data.as_slice())
        .unwrap()
        .amount;
    assert_eq!(vault_balance, credit_amount);

    let source_account = svm.get_account(&perp_usdc_ata).unwrap();
    let source_balance = TokenAccount::try_deserialize(&mut source_account.data.as_slice())
        .unwrap()
        .amount;
    assert_eq!(source_balance, 0);
}

// A random keypair that is NOT registered as pool.perp_program tries to credit — must be rejected.
#[test]
fn test_credit_unauthorized() {
    let perp_keypair = Keypair::new();
    let (mut svm, payer, pool, usdc_mint, usdc_vault, _lp_mint) = setup(perp_keypair.pubkey());
    let program_id = liquidity_pool::id();

    let intruder = Keypair::new();
    svm.airdrop(&intruder.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let credit_amount = 100 * 1_000_000u64;

    let intruder_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&intruder.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut svm, &payer, &usdc_mint, &intruder_ata, credit_amount)
        .send()
        .unwrap();

    let blockhash = svm.latest_blockhash();
    let credit_ix = make_credit_ix(
        program_id,
        intruder.pubkey(), // not pool.perp_program → Unauthorized
        pool,
        intruder_ata,
        usdc_mint,
        usdc_vault,
        credit_amount,
    );
    let msg = Message::new_with_blockhash(&[credit_ix], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &intruder])
            .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "expected unauthorized error");
}
