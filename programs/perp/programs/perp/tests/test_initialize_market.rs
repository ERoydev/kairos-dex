use anchor_lang::{
    prelude::Pubkey,
    solana_program::{instruction::Instruction, system_program},
    AccountDeserialize, InstructionData, ToAccountMetas,
};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use solana_sdk::native_token::LAMPORTS_PER_SOL;

use perp::{state::market::Market, MConfig, MARKET_SEED, GLOBAL_SEED};

fn program_bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/perp.so"))
}

fn market_pda(program_id: &Pubkey, sym: &[u8; 16]) -> Pubkey {
    Pubkey::find_program_address(&[MARKET_SEED, sym.as_ref()], program_id).0
}

fn global_config_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[GLOBAL_SEED], program_id).0
}

fn default_config() -> MConfig {
    MConfig {
        max_leverage: 20,
        max_open_interest: 1_000_000_000,
        open_interest_long: 0,
        open_interest_short: 0,
        mmr_bps: 500,
        open_fee_bps: 10,
        close_fee_bps: 10,
    }
}

fn symbol(s: &str) -> [u8; 16] {
    let mut buf = [0u8; 16];
    let bytes = s.as_bytes();
    buf[..bytes.len()].copy_from_slice(bytes);
    buf
}

fn send_ix(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> litesvm::types::TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn make_initialize_global_ix(program_id: Pubkey, payer: Pubkey, global_config: Pubkey, fee_receiver: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::InitializeGlobal { fee_receiver, max_markets: 10 }.data(),
        perp::accounts::InitializeGlobal { payer, global_config, system_program: system_program::ID }.to_account_metas(None),
    )
}

fn make_initialize_market_ix(
    program_id: Pubkey,
    payer: Pubkey,
    global_config: Pubkey,
    market: Pubkey,
    oracle: Pubkey,
    sym: [u8; 16],
    config: MConfig,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::InitializeMarket { symbol: sym, config }.data(),
        perp::accounts::InitializeMarket {
            payer,
            global_config,
            market,
            oracle,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn setup() -> (LiteSVM, Keypair, Pubkey, Pubkey) {
    let mut svm = LiteSVM::new();
    let program_id = perp::id();
    svm.add_program(program_id, program_bytes()).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    // initialize global config so market ix can read it
    let global_config = global_config_pda(&program_id);
    let fee_receiver = Keypair::new().pubkey();
    let ix = make_initialize_global_ix(program_id, payer.pubkey(), global_config, fee_receiver);
    send_ix(&mut svm, ix, &[&payer]).unwrap();

    (svm, payer, program_id, global_config)
}

#[test]
fn test_initialize_market_ok() {
    let (mut svm, payer, program_id, global_config) = setup();
    let sym = symbol("BTC-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&program_id, &sym);

    let ix = make_initialize_market_ix(program_id, payer.pubkey(), global_config, market, oracle, sym, default_config());
    assert!(send_ix(&mut svm, ix, &[&payer]).is_ok());

    let account = svm.get_account(&market).expect("market not found");
    let data = Market::try_deserialize(&mut account.data.as_slice()).unwrap();

    assert_eq!(data.symbol, sym);
    assert_eq!(data.authority, payer.pubkey());
    assert_eq!(data.oracle, oracle);
    assert_eq!(data.max_leverage, 20);
    assert_eq!(data.open_fee_bps, 10);
    assert_eq!(data.close_fee_bps, 10);
    assert_eq!(data.maintenance_margin_bps, 500);
    assert!(data.is_active);
}

#[test]
fn test_initialize_market_rejects_non_admin() {
    let (mut svm, _payer, program_id, global_config) = setup();
    let attacker = Keypair::new();
    svm.airdrop(&attacker.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

    let sym = symbol("BTC-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&program_id, &sym);

    let ix = make_initialize_market_ix(program_id, attacker.pubkey(), global_config, market, oracle, sym, default_config());
    assert!(send_ix(&mut svm, ix, &[&attacker]).is_err());
}

#[test]
fn test_initialize_market_rejects_zero_leverage() {
    let (mut svm, payer, program_id, global_config) = setup();
    let sym = symbol("ETH-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&program_id, &sym);

    let mut config = default_config();
    config.max_leverage = 0;

    let ix = make_initialize_market_ix(program_id, payer.pubkey(), global_config, market, oracle, sym, config);
    assert!(send_ix(&mut svm, ix, &[&payer]).is_err());
}

#[test]
fn test_initialize_market_rejects_fee_over_100_percent() {
    let (mut svm, payer, program_id, global_config) = setup();
    let sym = symbol("SOL-PERP");
    let oracle = Keypair::new().pubkey();
    let market = market_pda(&program_id, &sym);

    let mut config = default_config();
    config.open_fee_bps = 10_000;

    let ix = make_initialize_market_ix(program_id, payer.pubkey(), global_config, market, oracle, sym, config);
    assert!(send_ix(&mut svm, ix, &[&payer]).is_err());
}

#[test]
fn test_initialize_market_rejects_default_oracle() {
    let (mut svm, payer, program_id, global_config) = setup();
    let sym = symbol("ARB-PERP");
    let market = market_pda(&program_id, &sym);

    let ix = make_initialize_market_ix(program_id, payer.pubkey(), global_config, market, Pubkey::default(), sym, default_config());
    assert!(send_ix(&mut svm, ix, &[&payer]).is_err());
}
