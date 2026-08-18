// Integration test for `open_position`, using LiteSVM with both `perp.so` and
// `liquidity_pool.so` loaded so the real credit CPI path executes end to end.
//
// Accounts a single open_position call needs, in setup order:
//   1. global_config        (perp::initialize_global)
//   2. market + market_vault (perp::initialize_market)
//   3. lp_pool + lp_pool_usdc_vault + lp_mint (liquidity_pool::initialize_pool),
//      with pool.perp_program == perp::id() so the CPI auth check in `credit` passes
//   4. usdc_mint, trader_usdc_ata (funded), fee_receiver_ata
//   5. a fabricated PriceUpdateV2 account owned by the pyth receiver program id —
//      we don't run the real Pyth receiver program on-chain, so this is hand-built
//   6. position PDA (init'd by the instruction itself)
//
// Build prerequisite (not run by `cargo test` automatically):
//   `cargo build-sbf --features dev` in programs/perp/programs/perp, so
//   target/deploy/perp.so exists for `perp_bytes()` below. liquidity_pool.so is
//   pulled from the sibling liquidity-pool repo's own target/deploy — see
//   `liquidity_pool_bytes()`.
//
// STATUS: one passing happy-path test (long, under all caps). Not yet covered:
// short side, non-zero fee split (blocked by initialize_market's hardcoded
// mock_lp_tvl capping max_position_notional too low to produce fee >= 1), and
// failure cases (market paused, caps exceeded, stale price, unauthorized).

use anchor_lang::{
    prelude::Pubkey,
    solana_program::{instruction::Instruction, system_program},
    AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas,
};
use anchor_spl::token::TokenAccount;
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use pyth_solana_receiver_sdk::price_update::{
    get_feed_id_from_hex, PriceFeedMessage, PriceUpdateV2, VerificationLevel,
};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use perp::{
    state::{
        position::PositionType,
        syntetic_market::{SynteticMarket, TvlScaledCaps},
    },
    OpenPositionParams, SMParams, GLOBAL_SEED, MARKET_SEED, MARKET_VAULT, POSITION_SEED,
};

const USDC_DECIMALS: u8 = 6;
const FEED_ID_HEX: &str = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";

// --- program bytes -----------------------------------------------------

fn perp_bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/perp.so"))
}

fn liquidity_pool_bytes() -> &'static [u8] {
    include_bytes!("../../../../liquidity-pool/target/deploy/liquidity_pool.so")
}

// --- misc helpers --------------------------------------------------------

fn symbol(s: &str) -> [u8; 16] {
    let mut buf = [0u8; 16];
    let bytes = s.as_bytes();
    buf[..bytes.len()].copy_from_slice(bytes);
    buf
}

/// initialize_market hardcodes mock_lp_tvl = 50_000, which makes max_position_notional
/// a tiny 500 MicroUsdc — too low for base_fee_bps=10 to ever produce a non-zero fee
/// (needs notional >= 1000). To exercise the fee-split path we widen this market's
/// caps directly (bypassing that placeholder), leaving everything else untouched.
fn widen_market_caps(svm: &mut LiteSVM, market: &Pubkey, caps: TvlScaledCaps) {
    let mut account = svm.get_account(market).expect("market not found");
    let mut data = SynteticMarket::try_deserialize(&mut account.data.as_slice()).unwrap();
    data.risk_management.caps = caps;

    let mut new_data = Vec::new();
    data.try_serialize(&mut new_data).unwrap();
    account.data = new_data;
    svm.set_account(*market, account).unwrap();
}

fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let account = svm.get_account(ata).expect("token account not found");
    TokenAccount::try_deserialize(&mut account.data.as_slice())
        .unwrap()
        .amount
}

fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// --- PDA helpers -----------------------------------------------------------

fn global_config_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[GLOBAL_SEED], program_id).0
}

fn market_pda(program_id: &Pubkey, sym: &[u8; 16]) -> Pubkey {
    Pubkey::find_program_address(&[MARKET_SEED, sym.as_ref()], program_id).0
}

fn market_vault_pda(program_id: &Pubkey, market: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[MARKET_VAULT, market.as_ref()], program_id).0
}

fn position_pda(program_id: &Pubkey, trader: &Pubkey, market: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[POSITION_SEED, trader.as_ref(), market.as_ref()],
        program_id,
    )
    .0
}

fn lp_pool_pda(lp_program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[liquidity_pool::constants::LIQUIDITY_POOL_SEED],
        lp_program_id,
    )
    .0
}

fn lp_usdc_vault_pda(lp_program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[liquidity_pool::USDC_VAULT_SEED], lp_program_id).0
}

fn lp_mint_pda(lp_program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[liquidity_pool::LP_MINT_SEED], lp_program_id).0
}

// --- instruction builders ---------------------------------------------------

fn make_initialize_global_ix(
    program_id: Pubkey,
    payer: Pubkey,
    global_config: Pubkey,
    fee_receiver: Pubkey,
    max_markets: u16,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::InitializeGlobal {
            fee_receiver,
            max_markets,
        }
        .data(),
        perp::accounts::InitializeGlobal {
            payer,
            global_config,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn make_initialize_market_ix(
    program_id: Pubkey,
    payer: Pubkey,
    global_config: Pubkey,
    market: Pubkey,
    vault: Pubkey,
    oracle: Pubkey,
    usdc_mint: Pubkey,
    sym: [u8; 16],
    config: SMParams,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::InitializeMarket {
            symbol: sym,
            config,
        }
        .data(),
        perp::accounts::InitializeMarket {
            payer,
            global_config,
            market,
            vault,
            oracle,
            usdc_mint,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn make_initialize_pool_ix(
    lp_program_id: Pubkey,
    authority: Pubkey,
    pool: Pubkey,
    usdc_mint: Pubkey,
    usdc_vault: Pubkey,
    lp_mint: Pubkey,
    perp_program: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        lp_program_id,
        &liquidity_pool::instruction::InitializePool {}.data(),
        liquidity_pool::accounts::InitializePool {
            pool,
            authority,
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

#[allow(clippy::too_many_arguments)]
fn make_open_position_ix(
    program_id: Pubkey,
    lp_program_id: Pubkey,
    trader: Pubkey,
    price_update: Pubkey,
    global_config: Pubkey,
    position: Pubkey,
    market_vault: Pubkey,
    market: Pubkey,
    lp_pool: Pubkey,
    lp_pool_usdc_vault: Pubkey,
    fee_receiver_ata: Pubkey,
    trader_usdc_ata: Pubkey,
    usdc_mint: Pubkey,
    params: OpenPositionParams,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::OpenPosition { o_params: params }.data(),
        perp::accounts::OpenPosition {
            trader,
            price_update,
            global_config,
            position,
            market_vault,
            market,
            lp_pool,
            lp_pool_usdc_vault,
            lp_pool_program: lp_program_id,
            fee_receiver_ata,
            trader_usdc_ata,
            usdc_mint,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

// --- fabricated Pyth price account -----------------------------------------

/// Builds a `PriceUpdateV2` account by hand and injects it into LiteSVM via
/// `set_account`, so `_open_position`'s `get_price_no_older_than` call succeeds
/// without running the real Pyth receiver program on-chain. Uses the SVM's own
/// current clock as `publish_time` so the 30s freshness check always passes.
fn fabricate_price_update(svm: &mut LiteSVM, price: i64) -> Pubkey {
    let feed_id = get_feed_id_from_hex(FEED_ID_HEX).unwrap();
    let publish_time = svm
        .get_sysvar::<anchor_lang::solana_program::clock::Clock>()
        .unix_timestamp;

    let price_update = PriceUpdateV2 {
        write_authority: Pubkey::default(),
        verification_level: VerificationLevel::Full,
        price_message: PriceFeedMessage {
            feed_id,
            price,
            conf: 1,
            exponent: -8,
            publish_time,
            prev_publish_time: publish_time,
            ema_price: price,
            ema_conf: 1,
        },
        posted_slot: 0,
    };

    let mut data = Vec::new();
    price_update.try_serialize(&mut data).unwrap();

    let address = Pubkey::new_unique();
    svm.set_account(
        address,
        solana_account::Account {
            lamports: LAMPORTS_PER_SOL,
            data,
            owner: pyth_solana_receiver_sdk::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    address
}

// --- environment setup -------------------------------------------------------

/// Bundle of everything a test needs to fire an `open_position` ix.
struct Env {
    svm: LiteSVM,
    trader: Keypair,
    program_id: Pubkey,
    lp_program_id: Pubkey,
    global_config: Pubkey,
    market: Pubkey,
    market_vault: Pubkey,
    lp_pool: Pubkey,
    lp_pool_usdc_vault: Pubkey,
    usdc_mint: Pubkey,
    trader_usdc_ata: Pubkey,
    fee_receiver_ata: Pubkey,
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

    // market + market_vault
    let sym = symbol("BTC-PERP");
    let market = market_pda(&program_id, &sym);
    let market_vault = market_vault_pda(&program_id, &market);
    let oracle = Keypair::new().pubkey();
    let init_market_ix = make_initialize_market_ix(
        program_id,
        payer.pubkey(),
        global_config,
        market,
        market_vault,
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

    // lp_pool — perp_program must be market_vault's address: that's the account
    // that actually signs the credit CPI (via invoke_signed on its own PDA seeds),
    // not the perp program id itself. See liquidity_pool::instructions::credit.
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

    // trader + fee receiver ATAs
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

    let fee_receiver_ata = CreateAssociatedTokenAccount::new(&mut svm, &payer, &usdc_mint)
        .owner(&fee_receiver)
        .send()
        .unwrap();

    Env {
        svm,
        trader,
        program_id,
        lp_program_id,
        global_config,
        market,
        market_vault,
        lp_pool,
        lp_pool_usdc_vault,
        usdc_mint,
        trader_usdc_ata,
        fee_receiver_ata,
    }
}

// --- tests -------------------------------------------------------------------

#[test]
fn test_open_position_ok() {
    let mut env = setup();
    let price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00 @ -8 exponent

    let position = position_pda(&env.program_id, &env.trader.pubkey(), &env.market);
    // Market caps are derived from initialize_market's hardcoded mock_lp_tvl = 50_000
    // (MicroUsdc), which makes max_position_notional a tiny 500 MicroUsdc — so these
    // amounts are deliberately small to stay under cap, not representative USDC sizes.
    let params = OpenPositionParams {
        leverage: 4,
        margin: 100,
        take_profit: 0,
        stop_loss: 0,
        position_type: PositionType::Long,
    };

    let trader_ata_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let market_vault_before = token_balance(&env.svm, &env.market_vault);
    let fee_receiver_before = token_balance(&env.svm, &env.fee_receiver_ata);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let lp_pool_assets_before = liquidity_pool::Pool::try_deserialize(
        &mut env.svm.get_account(&env.lp_pool).unwrap().data.as_slice(),
    )
    .unwrap()
    .total_assets;

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

    let account = env.svm.get_account(&position).expect("position not found");
    let data =
        perp::state::position::Position::try_deserialize(&mut account.data.as_slice()).unwrap();

    assert_eq!(data.owner, env.trader.pubkey());
    assert_eq!(data.market, env.market);
    assert!(data.side == PositionType::Long);
    assert_eq!(data.notional, 400); // margin(100) * leverage(4), fee floors to 0 at this size
    assert_eq!(data.collateral, 100);
    assert_eq!(data.entry_price, 100 * 1_000_000); // $100.00 in MicroUsdc

    let market_account = env.svm.get_account(&env.market).unwrap();
    let market_data = SynteticMarket::try_deserialize(&mut market_account.data.as_slice()).unwrap();
    assert_eq!(market_data.oi_long, 400);
    assert_eq!(market_data.oi_short, 0);

    // Token movements. fee_to_pay is 0 at this notional (10bps of 400 floors to 0),
    // so trader only ever loses the margin itself, and the fee-split legs (protocol
    // + LP) are exercised as zero-amount transfers, not skipped — this does NOT
    // prove the fee-split arithmetic is correct for a non-zero fee, only that the
    // transfer wiring/authorities are right. See PR notes: max_position_notional is
    // capped at 500 by initialize_market's hardcoded mock_lp_tvl, so no amount under
    // that cap can produce fee >= 1 (needs notional >= 1000 for base_fee_bps=10).
    assert_eq!(
        trader_ata_before - token_balance(&env.svm, &env.trader_usdc_ata),
        100
    );
    assert_eq!(
        token_balance(&env.svm, &env.market_vault) - market_vault_before,
        100
    );
    assert_eq!(
        token_balance(&env.svm, &env.fee_receiver_ata),
        fee_receiver_before
    );
    assert_eq!(
        token_balance(&env.svm, &env.lp_pool_usdc_vault),
        lp_vault_before
    );

    let lp_pool_assets_after = liquidity_pool::Pool::try_deserialize(
        &mut env.svm.get_account(&env.lp_pool).unwrap().data.as_slice(),
    )
    .unwrap()
    .total_assets;
    assert_eq!(lp_pool_assets_after, lp_pool_assets_before);
}

/// Same setup, but with market caps widened so the trade actually produces a
/// non-zero fee, so we can verify the protocol/LP fee split actually lands in
/// the right accounts for the right amounts (not just that the transfers no-op).
#[test]
fn test_open_position_distributes_fees() {
    let mut env = setup();
    widen_market_caps(
        &mut env.svm,
        &env.market,
        TvlScaledCaps {
            max_position_notional: 1_000_000_000,
            max_user_notional: 1_000_000_000,
            max_oi_long: 1_000_000_000,
            max_oi_short: 1_000_000_000,
            max_skew: 20_000_000, // small relative to notional below, so skew_fee also kicks in
        },
    );
    let price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00 @ -8 exponent

    let position = position_pda(&env.program_id, &env.trader.pubkey(), &env.market);
    // margin=1_000_000 (1 USDC), leverage=10 -> notional_before_fee=10_000_000.
    // base_fee = 10bps of 10_000_000 = 10_000. Skew goes 0 -> 10_000_000 against a
    // 20_000_000 cap (worsens_skew=true), so skew_fee = 10_000_000/20_000_000 * 20bps
    // of notional = 10bps of 10_000_000 = 10_000. total fee = 20_000.
    // protocol_fees = 15% of 20_000 = 3_000, lp_fees = 17_000.
    let params = OpenPositionParams {
        leverage: 10,
        margin: 1_000_000,
        take_profit: 0,
        stop_loss: 0,
        position_type: PositionType::Long,
    };
    let margin = params.margin;
    let expected_fee = 20_000u64;
    let expected_protocol_fee = 3_000u64;
    let expected_lp_fee = 17_000u64;
    let expected_collateral = margin - expected_fee;

    let trader_ata_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let market_vault_before = token_balance(&env.svm, &env.market_vault);
    let fee_receiver_before = token_balance(&env.svm, &env.fee_receiver_ata);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let lp_pool_assets_before = liquidity_pool::Pool::try_deserialize(
        &mut env.svm.get_account(&env.lp_pool).unwrap().data.as_slice(),
    )
    .unwrap()
    .total_assets;

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

    let account = env.svm.get_account(&position).expect("position not found");
    let data =
        perp::state::position::Position::try_deserialize(&mut account.data.as_slice()).unwrap();
    assert_eq!(data.owner, env.trader.pubkey());
    assert_eq!(data.market, env.market);
    assert!(data.side == PositionType::Long);
    assert_eq!(data.entry_price, 100 * 1_000_000);
    assert_eq!(data.collateral, expected_collateral);
    assert_eq!(data.notional, expected_collateral * 10);

    let market_account = env.svm.get_account(&env.market).unwrap();
    let market_data = SynteticMarket::try_deserialize(&mut market_account.data.as_slice()).unwrap();
    assert_eq!(market_data.oi_long, expected_collateral * 10);
    assert_eq!(market_data.oi_short, 0);

    // Trader's transfers cover collateral + both fee cuts: transfer_margin_to_vault
    // sends (margin + lp_fee) so the vault can forward lp_fee on to the pool via
    // credit_lp_pool while still being left holding exactly `margin`, and
    // transfer_protocol_fee sends protocol_fees directly. Total trader outflow is
    // therefore the full original margin.
    assert_eq!(
        trader_ata_before - token_balance(&env.svm, &env.trader_usdc_ata),
        margin
    );
    assert_eq!(
        token_balance(&env.svm, &env.market_vault) - market_vault_before,
        expected_collateral
    );
    assert_eq!(
        token_balance(&env.svm, &env.fee_receiver_ata) - fee_receiver_before,
        expected_protocol_fee
    );
    assert_eq!(
        token_balance(&env.svm, &env.lp_pool_usdc_vault) - lp_vault_before,
        expected_lp_fee
    );

    let lp_pool_assets_after = liquidity_pool::Pool::try_deserialize(
        &mut env.svm.get_account(&env.lp_pool).unwrap().data.as_slice(),
    )
    .unwrap()
    .total_assets;
    assert_eq!(
        lp_pool_assets_after - lp_pool_assets_before,
        expected_lp_fee
    );
}
