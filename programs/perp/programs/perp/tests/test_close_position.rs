// Integration test for `close_position`, using LiteSVM with both `perp.so` and
// `liquidity_pool.so` loaded so the credit/debit CPI paths execute end to end.
//
// Each test opens a position first (via open_position), then closes it and
// asserts token movements across trader / market_vault / fee_receiver / lp_pool.
//
// Build prerequisite (not run by `cargo test` automatically):
//   `cargo build-sbf --features dev` in programs/perp/programs/perp, so
//   target/deploy/perp.so exists for `perp_bytes()` below. liquidity_pool.so is
//   pulled from the sibling liquidity-pool repo's own target/deploy.
//
// Covers three settlement branches of `settle()`:
//   - breakeven (payout == collateral, no pool interaction)
//   - full loss (payout == 0, entire collateral credited to pool)
//   - profit beyond own collateral (payout split: vault covers the trader's own
//     collateral, pool covers the rest via `debit`) — this is the branch that
//     had a double-payment bug (vault_payout wasn't subtracting debit_from_pool).

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
/// a tiny 500 MicroUsdc. Widen caps directly (bypassing that placeholder) so a test
/// can open a large enough position to produce a non-zero fee and meaningful PnL.
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

fn pool_total_assets(svm: &LiteSVM, pool: &Pubkey) -> u64 {
    liquidity_pool::Pool::try_deserialize(&mut svm.get_account(pool).unwrap().data.as_slice())
        .unwrap()
        .total_assets
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
fn make_deposit_ix(
    lp_program_id: Pubkey,
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
        lp_program_id,
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

#[allow(clippy::too_many_arguments)]
fn make_close_position_ix(
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
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::ClosePosition {}.data(),
        perp::accounts::ClosePosition {
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
        }
        .to_account_metas(None),
    )
}

// --- fabricated Pyth price account -----------------------------------------

/// Builds a `PriceUpdateV2` account by hand and injects it into LiteSVM, so
/// `get_price_no_older_than` succeeds without running the real Pyth receiver
/// program on-chain. Uses the SVM's own current clock as `publish_time` so the
/// 30s freshness check always passes.
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
    lp_mint: Pubkey,
    usdc_mint: Pubkey,
    trader_usdc_ata: Pubkey,
    fee_receiver_ata: Pubkey,
    payer: Keypair,
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
    // that actually signs the credit/debit CPI (via invoke_signed on its own PDA
    // seeds), not the perp program id itself. See liquidity_pool::instructions::credit.
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
        lp_mint,
        usdc_mint,
        trader_usdc_ata,
        fee_receiver_ata,
        payer,
    }
}

/// Funds the LP pool's vault with `amount` USDC from a fresh depositor, so a
/// close_position test can exercise the `debit` (pool pays trader) path.
fn fund_pool(env: &mut Env, amount: u64) {
    let provider = Keypair::new();
    env.svm
        .airdrop(&provider.pubkey(), 10 * LAMPORTS_PER_SOL)
        .unwrap();

    let provider_ata = CreateAssociatedTokenAccount::new(&mut env.svm, &env.payer, &env.usdc_mint)
        .owner(&provider.pubkey())
        .send()
        .unwrap();
    MintTo::new(
        &mut env.svm,
        &env.payer,
        &env.usdc_mint,
        &provider_ata,
        amount,
    )
    .send()
    .unwrap();

    let provider_lp_ata = anchor_spl::associated_token::get_associated_token_address(
        &provider.pubkey(),
        &env.lp_mint,
    );

    let deposit_ix = make_deposit_ix(
        env.lp_program_id,
        provider.pubkey(),
        env.lp_pool,
        provider_ata,
        provider_lp_ata,
        env.usdc_mint,
        env.lp_pool_usdc_vault,
        env.lp_mint,
        amount,
    );
    let res = send_ix(&mut env.svm, deposit_ix, &[&provider]);
    assert!(res.is_ok(), "deposit failed: {:?}", res.err());
}

/// Opens a position for `env.trader` and returns its PDA.
fn open(env: &mut Env, price_update: Pubkey, params: OpenPositionParams) -> Pubkey {
    let position = position_pda(&env.program_id, &env.trader.pubkey(), &env.market);
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
    position
}

// --- tests -------------------------------------------------------------------

/// Same entry and exit price -> zero PnL, and the position is small enough that
/// the flat close fee floors to zero too. Trader should get exactly their
/// collateral back, nothing should touch the LP pool, and the position account
/// should be closed (rent refunded).
#[test]
fn test_close_position_breakeven() {
    let mut env = setup();
    let price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00

    let position = open(
        &mut env,
        price_update,
        OpenPositionParams {
            leverage: 4,
            margin: 100,
            take_profit: 0,
            stop_loss: 0,
            position_type: PositionType::Long,
        },
    );

    let trader_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let vault_before = token_balance(&env.svm, &env.market_vault);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let pool_assets_before = pool_total_assets(&env.svm, &env.lp_pool);

    let close_ix = make_close_position_ix(
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
    );
    let res = send_ix(&mut env.svm, close_ix, &[&env.trader]);
    assert!(res.is_ok(), "close_position failed: {:?}", res.err());

    // Trader gets their full collateral back (100), vault drains by exactly that,
    // and the pool/fee receiver are untouched since fee floored to 0.
    assert_eq!(
        token_balance(&env.svm, &env.trader_usdc_ata) - trader_before,
        100
    );
    assert_eq!(
        vault_before - token_balance(&env.svm, &env.market_vault),
        100
    );
    assert_eq!(
        token_balance(&env.svm, &env.lp_pool_usdc_vault),
        lp_vault_before
    );
    assert_eq!(
        pool_total_assets(&env.svm, &env.lp_pool),
        pool_assets_before
    );

    let market_data = SynteticMarket::try_deserialize(
        &mut env.svm.get_account(&env.market).unwrap().data.as_slice(),
    )
    .unwrap();
    assert_eq!(market_data.oi_long, 0);

    let closed = env
        .svm
        .get_account(&position)
        .map(|a| a.lamports)
        .unwrap_or(0);
    assert_eq!(closed, 0, "position account should be closed / drained");
}

/// A 10% adverse price move against 10x leverage wipes the position out exactly.
/// Trader should get nothing back, and the vault's entire remaining collateral
/// (after the close fee split) should be credited to the LP pool.
#[test]
fn test_close_position_full_loss_credits_pool() {
    let mut env = setup();
    widen_market_caps(
        &mut env.svm,
        &env.market,
        TvlScaledCaps {
            max_position_notional: 1_000_000_000,
            max_user_notional: 1_000_000_000,
            max_oi_long: 1_000_000_000,
            max_oi_short: 1_000_000_000,
            max_skew: 20_000_000,
        },
    );
    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00

    // margin=1_000_000, leverage=10 -> open fee 20_000 (base+skew), collateral 980_000,
    // notional 9_800_000. Matches test_open_position_distributes_fees's math.
    let position = open(
        &mut env,
        open_price_update,
        OpenPositionParams {
            leverage: 10,
            margin: 1_000_000,
            take_profit: 0,
            stop_loss: 0,
            position_type: PositionType::Long,
        },
    );

    // close_fee = 10bps of notional 9_800_000 = 9_800 -> protocol 1_470 / lp 8_330.
    // collateral_after_fee = 980_000 - 9_800 = 970_200.
    // -10% move: pnl = -10_000_000/100_000_000 * 9_800_000 = -980_000, which wipes
    // out even the pre-fee collateral, so payout=0 and the pool eats the rest.
    let close_price_update = fabricate_price_update(&mut env.svm, 90_00_000_000); // $90.00

    let trader_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let vault_before = token_balance(&env.svm, &env.market_vault);
    let fee_receiver_before = token_balance(&env.svm, &env.fee_receiver_ata);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let pool_assets_before = pool_total_assets(&env.svm, &env.lp_pool);

    let close_ix = make_close_position_ix(
        env.program_id,
        env.lp_program_id,
        env.trader.pubkey(),
        close_price_update,
        env.global_config,
        position,
        env.market_vault,
        env.market,
        env.lp_pool,
        env.lp_pool_usdc_vault,
        env.fee_receiver_ata,
        env.trader_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, close_ix, &[&env.trader]);
    assert!(res.is_ok(), "close_position failed: {:?}", res.err());

    let expected_protocol_fee = 1_470u64;
    let expected_lp_fee = 8_330u64;
    let expected_credit_to_pool = 970_200u64;

    // Trader receives nothing.
    assert_eq!(token_balance(&env.svm, &env.trader_usdc_ata), trader_before);
    // Vault drains by exactly the position's remaining collateral (980_000 total:
    // fee split + full loss credit), leaving no leftover or shortfall.
    assert_eq!(
        vault_before - token_balance(&env.svm, &env.market_vault),
        980_000
    );
    assert_eq!(
        token_balance(&env.svm, &env.fee_receiver_ata) - fee_receiver_before,
        expected_protocol_fee
    );
    assert_eq!(
        token_balance(&env.svm, &env.lp_pool_usdc_vault) - lp_vault_before,
        expected_lp_fee + expected_credit_to_pool
    );
    assert_eq!(
        pool_total_assets(&env.svm, &env.lp_pool) - pool_assets_before,
        expected_lp_fee + expected_credit_to_pool
    );
}

/// A +10% favorable move produces profit bigger than the trader's own collateral
/// could cover, so the pool must fund the excess via `debit`. This is the
/// regression test for the double-payment bug: the vault must pay only its own
/// share (collateral_after_fee), not the full payout, or the trader gets paid
/// twice and the vault gets over-drained.
#[test]
fn test_close_position_profit_funded_by_pool() {
    let mut env = setup();
    widen_market_caps(
        &mut env.svm,
        &env.market,
        TvlScaledCaps {
            max_position_notional: 1_000_000_000,
            max_user_notional: 1_000_000_000,
            max_oi_long: 1_000_000_000,
            max_oi_short: 1_000_000_000,
            max_skew: 20_000_000,
        },
    );
    // Seed the pool with enough liquidity to cover the payout below (980_000).
    fund_pool(&mut env, 5_000_000);

    let open_price_update = fabricate_price_update(&mut env.svm, 100_00_000_000); // $100.00
    let position = open(
        &mut env,
        open_price_update,
        OpenPositionParams {
            leverage: 10,
            margin: 1_000_000,
            take_profit: 0,
            stop_loss: 0,
            position_type: PositionType::Long,
        },
    );

    // close_fee = 9_800 -> protocol 1_470 / lp 8_330. collateral_after_fee = 970_200.
    // +10% move: pnl = +980_000. net = 970_200 + 980_000 = 1_950_200, which exceeds
    // collateral_after_fee, so debit_from_pool = 1_950_200 - 970_200 = 980_000 and
    // the vault only ever pays out its own 970_200 share.
    let close_price_update = fabricate_price_update(&mut env.svm, 110_00_000_000); // $110.00

    let trader_before = token_balance(&env.svm, &env.trader_usdc_ata);
    let vault_before = token_balance(&env.svm, &env.market_vault);
    let fee_receiver_before = token_balance(&env.svm, &env.fee_receiver_ata);
    let lp_vault_before = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    let pool_assets_before = pool_total_assets(&env.svm, &env.lp_pool);

    let close_ix = make_close_position_ix(
        env.program_id,
        env.lp_program_id,
        env.trader.pubkey(),
        close_price_update,
        env.global_config,
        position,
        env.market_vault,
        env.market,
        env.lp_pool,
        env.lp_pool_usdc_vault,
        env.fee_receiver_ata,
        env.trader_usdc_ata,
        env.usdc_mint,
    );
    let res = send_ix(&mut env.svm, close_ix, &[&env.trader]);
    assert!(res.is_ok(), "close_position failed: {:?}", res.err());

    let expected_protocol_fee = 1_470u64;
    let expected_lp_fee = 8_330u64;
    let expected_vault_payout = 970_200u64;
    let expected_debit_from_pool = 980_000u64;
    let expected_total_payout = expected_vault_payout + expected_debit_from_pool; // 1_950_200

    // Trader receives exactly the total payout — not more (the bug would have
    // added an extra 980_000 on top by also paying the full `payout` from the vault).
    assert_eq!(
        token_balance(&env.svm, &env.trader_usdc_ata) - trader_before,
        expected_total_payout
    );
    // Vault only loses its own share: the fee split plus the trader's own collateral,
    // never the pool-funded profit portion.
    assert_eq!(
        vault_before - token_balance(&env.svm, &env.market_vault),
        expected_protocol_fee + expected_lp_fee + expected_vault_payout
    );
    assert_eq!(
        token_balance(&env.svm, &env.fee_receiver_ata) - fee_receiver_before,
        expected_protocol_fee
    );
    // Pool vault: +lp_fee from the close-fee credit, -debit_from_pool paid to trader.
    let lp_vault_after = token_balance(&env.svm, &env.lp_pool_usdc_vault);
    assert_eq!(
        lp_vault_before + expected_lp_fee - lp_vault_after,
        expected_debit_from_pool
    );
    let pool_assets_after = pool_total_assets(&env.svm, &env.lp_pool);
    assert_eq!(
        pool_assets_before + expected_lp_fee - pool_assets_after,
        expected_debit_from_pool
    );
}
