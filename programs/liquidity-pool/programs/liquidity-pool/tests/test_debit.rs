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

fn make_debit_ix(
    program_id: Pubkey,
    caller: Pubkey,
    pool: Pubkey,
    usdc_mint: Pubkey,
    usdc_vault: Pubkey,
    destination: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &liquidity_pool::instruction::Debit { amount }.data(),
        liquidity_pool::accounts::Debit {
            caller,
            pool,
            usdc_vault,
            destination,
            usdc_mint,
            token_program: anchor_spl::token::ID,
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

// Deposits USDC into the pool via an LP provider so debit tests have funds to draw from.
fn fund_pool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    pool: Pubkey,
    usdc_mint: Pubkey,
    usdc_vault: Pubkey,
    lp_mint: Pubkey,
    amount: u64,
) {
    let program_id = liquidity_pool::id();

    let provider_ata = CreateAssociatedTokenAccount::new(svm, payer, &usdc_mint)
        .owner(&payer.pubkey())
        .send()
        .unwrap();
    MintTo::new(svm, payer, &usdc_mint, &provider_ata, amount)
        .send()
        .unwrap();

    let provider_lp_ata = get_ata(&payer.pubkey(), &lp_mint);

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
        amount,
    );
    let msg = Message::new_with_blockhash(&[deposit_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).unwrap();
}

// Happy path: perp keypair (acting as the perp program) pulls USDC out of the pool to a trader.
// In production this is called when a trader wins — the perp program sends their profit from the pool.
// We simulate the perp program by using a keypair whose pubkey == pool.perp_program.
#[test]
fn test_debit() {
    // perp_keypair simulates the perp program. Its pubkey is registered in pool.perp_program at init,
    // so it is the only caller authorized to credit/debit.
    let perp_keypair = Keypair::new();
    let (mut svm, payer, pool, usdc_mint, usdc_vault, lp_mint) = setup(perp_keypair.pubkey());
    let program_id = liquidity_pool::id();

    svm.airdrop(&perp_keypair.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let pool_funds = 1_000 * 1_000_000u64; // 1000 USDC deposited by LP provider
    let debit_amount = 300 * 1_000_000u64;  // 300 USDC paid out to winning trader

    // Seed the pool with funds via a normal LP deposit so there is something to debit.
    fund_pool(&mut svm, &payer, pool, usdc_mint, usdc_vault, lp_mint, pool_funds);

    // trader_ata is where the pool sends the payout (simulates winning trader's wallet).
    let trader = Keypair::new();
    let trader_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&trader.pubkey())
        .send()
        .unwrap();

    let blockhash = svm.latest_blockhash();
    let debit_ix = make_debit_ix(
        program_id,
        perp_keypair.pubkey(),
        pool,
        usdc_mint,
        usdc_vault,
        trader_ata,
        debit_amount,
    );
    let msg = Message::new_with_blockhash(&[debit_ix], Some(&payer.pubkey()), &blockhash);
    // Both payer (fee payer) and perp_keypair (caller/signer) must sign.
    // In production, perp_keypair is replaced by the perp program signing via invoke_signed.
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &perp_keypair])
            .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "debit failed: {:?}", res.err());

    let pool_account = svm.get_account(&pool).unwrap();
    let pool_data = Pool::try_deserialize(&mut pool_account.data.as_slice()).unwrap();
    assert_eq!(pool_data.total_assets, pool_funds - debit_amount);

    let trader_account = svm.get_account(&trader_ata).unwrap();
    let trader_balance = TokenAccount::try_deserialize(&mut trader_account.data.as_slice())
        .unwrap()
        .amount;
    assert_eq!(trader_balance, debit_amount);
}

// A random keypair that is NOT registered as pool.perp_program tries to debit — must be rejected.
#[test]
fn test_debit_unauthorized() {
    let perp_keypair = Keypair::new();
    let (mut svm, payer, pool, usdc_mint, usdc_vault, lp_mint) = setup(perp_keypair.pubkey());
    let program_id = liquidity_pool::id();

    fund_pool(&mut svm, &payer, pool, usdc_mint, usdc_vault, lp_mint, 1_000 * 1_000_000u64);

    let intruder = Keypair::new();
    svm.airdrop(&intruder.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let destination = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&intruder.pubkey())
        .send()
        .unwrap();

    let blockhash = svm.latest_blockhash();
    let debit_ix = make_debit_ix(
        program_id,
        intruder.pubkey(), // not pool.perp_program → Unauthorized
        pool,
        usdc_mint,
        usdc_vault,
        destination,
        100 * 1_000_000u64,
    );
    let msg = Message::new_with_blockhash(&[debit_ix], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &intruder])
            .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "expected unauthorized error");
}

// Debit more than pool.total_assets — must be rejected by the solvency check.
#[test]
fn test_debit_insufficient_funds() {
    let perp_keypair = Keypair::new();
    let (mut svm, payer, pool, usdc_mint, usdc_vault, lp_mint) = setup(perp_keypair.pubkey());
    let program_id = liquidity_pool::id();

    svm.airdrop(&perp_keypair.pubkey(), LAMPORTS_PER_SOL).unwrap();

    let pool_funds = 100 * 1_000_000u64;
    fund_pool(&mut svm, &payer, pool, usdc_mint, usdc_vault, lp_mint, pool_funds);

    let trader = Keypair::new();
    let destination = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&trader.pubkey())
        .send()
        .unwrap();

    let blockhash = svm.latest_blockhash();
    let debit_ix = make_debit_ix(
        program_id,
        perp_keypair.pubkey(),
        pool,
        usdc_mint,
        usdc_vault,
        destination,
        pool_funds + 1, // one lamport more than the pool holds
    );
    let msg = Message::new_with_blockhash(&[debit_ix], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &perp_keypair])
            .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "expected insufficient funds error");
}
