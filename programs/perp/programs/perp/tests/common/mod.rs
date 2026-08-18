// Shared test helpers for the `perp` integration tests: program byte loaders, PDA
// derivation, instruction builders, and small LiteSVM assertions/fixtures. Each
// `tests/*.rs` file is compiled as its own crate, so this lives under
// `tests/common/mod.rs` (not `tests/common.rs`) to avoid Cargo treating it as a
// separate test binary, and is pulled in via `mod common;`.
//
// Not every consumer uses every helper here, so unused-function warnings are
// expected per test binary and suppressed below.
#![allow(dead_code)]

use anchor_lang::{
    prelude::Pubkey,
    solana_program::{instruction::Instruction, system_program},
    AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas,
};
use anchor_spl::token::TokenAccount;
use litesvm::LiteSVM;
use litesvm_token::{CreateAssociatedTokenAccount, MintTo};
use pyth_solana_receiver_sdk::price_update::{
    get_feed_id_from_hex, PriceFeedMessage, PriceUpdateV2, VerificationLevel,
};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use perp::{
    state::syntetic_market::{SynteticMarket, TvlScaledCaps},
    OpenPositionParams, SMParams, GLOBAL_SEED, INSURANCE_FUND_VAULT, MARKET_SEED, MARKET_VAULT,
    POSITION_SEED,
};

pub const USDC_DECIMALS: u8 = 6;
pub const FEED_ID_HEX: &str = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";

// --- program bytes -----------------------------------------------------

pub fn perp_bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/perp.so"))
}

pub fn liquidity_pool_bytes() -> &'static [u8] {
    include_bytes!("../../../../../liquidity-pool/target/deploy/liquidity_pool.so")
}

// --- misc helpers --------------------------------------------------------

pub fn symbol(s: &str) -> [u8; 16] {
    let mut buf = [0u8; 16];
    let bytes = s.as_bytes();
    buf[..bytes.len()].copy_from_slice(bytes);
    buf
}

/// Default `SMParams` used across tests that don't care about specific config values.
pub fn default_config() -> SMParams {
    SMParams {
        max_leverage: 20,
        mmr_bps: 500,
        feed_id: format!("0x{FEED_ID_HEX}"),
    }
}

pub fn send_ix(
    svm: &mut LiteSVM,
    ix: Instruction,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

pub fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    let account = svm.get_account(ata).expect("token account not found");
    TokenAccount::try_deserialize(&mut account.data.as_slice())
        .unwrap()
        .amount
}

pub fn pool_total_assets(svm: &LiteSVM, pool: &Pubkey) -> u64 {
    liquidity_pool::Pool::try_deserialize(&mut svm.get_account(pool).unwrap().data.as_slice())
        .unwrap()
        .total_assets
}

/// Overwrites a market's risk caps directly, bypassing whatever TVL they were
/// seeded with at `initialize_market` time. Used by tests that need caps wide
/// enough to exercise the fee-split / PnL paths without a large LP deposit.
pub fn widen_market_caps(svm: &mut LiteSVM, market: &Pubkey, caps: TvlScaledCaps) {
    let mut account = svm.get_account(market).expect("market not found");
    let mut data = SynteticMarket::try_deserialize(&mut account.data.as_slice()).unwrap();
    data.risk_management.caps = caps;

    let mut new_data = Vec::new();
    data.try_serialize(&mut new_data).unwrap();
    account.data = new_data;
    svm.set_account(*market, account).unwrap();
}

/// Deposits `amount` USDC into the LP pool from a freshly-funded depositor. Used
/// both to seed a market's initial TVL-scaled caps before `initialize_market`,
/// and to fund the pool so `debit` has something to pay out from.
pub fn seed_lp_pool(
    svm: &mut LiteSVM,
    payer: &Keypair,
    lp_program_id: Pubkey,
    usdc_mint: Pubkey,
    lp_pool: Pubkey,
    lp_pool_usdc_vault: Pubkey,
    lp_mint: Pubkey,
    amount: u64,
) {
    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 10 * LAMPORTS_PER_SOL)
        .unwrap();
    let depositor_ata = CreateAssociatedTokenAccount::new(svm, payer, &usdc_mint)
        .owner(&depositor.pubkey())
        .send()
        .unwrap();
    MintTo::new(svm, payer, &usdc_mint, &depositor_ata, amount)
        .send()
        .unwrap();

    let depositor_lp_ata =
        anchor_spl::associated_token::get_associated_token_address(&depositor.pubkey(), &lp_mint);
    let deposit_ix = make_deposit_ix(
        lp_program_id,
        depositor.pubkey(),
        lp_pool,
        depositor_ata,
        depositor_lp_ata,
        usdc_mint,
        lp_pool_usdc_vault,
        lp_mint,
        amount,
    );
    let res = send_ix(svm, deposit_ix, &[&depositor]);
    assert!(res.is_ok(), "lp pool deposit failed: {:?}", res.err());
}

// --- PDA helpers -----------------------------------------------------------

pub fn global_config_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[GLOBAL_SEED], program_id).0
}

pub fn market_pda(program_id: &Pubkey, sym: &[u8; 16]) -> Pubkey {
    Pubkey::find_program_address(&[MARKET_SEED, sym.as_ref()], program_id).0
}

pub fn market_vault_pda(program_id: &Pubkey, market: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[MARKET_VAULT, market.as_ref()], program_id).0
}

pub fn insurance_fund_vault_pda(program_id: &Pubkey, market: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[INSURANCE_FUND_VAULT, market.as_ref()], program_id).0
}

pub fn position_pda(program_id: &Pubkey, trader: &Pubkey, market: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[POSITION_SEED, trader.as_ref(), market.as_ref()],
        program_id,
    )
    .0
}

pub fn lp_pool_pda(lp_program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[liquidity_pool::constants::LIQUIDITY_POOL_SEED],
        lp_program_id,
    )
    .0
}

pub fn lp_usdc_vault_pda(lp_program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[liquidity_pool::USDC_VAULT_SEED], lp_program_id).0
}

pub fn lp_mint_pda(lp_program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[liquidity_pool::LP_MINT_SEED], lp_program_id).0
}

// --- instruction builders ---------------------------------------------------

pub fn make_initialize_global_ix(
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

pub fn make_update_global_ix(
    program_id: Pubkey,
    authority: Pubkey,
    global_config: Pubkey,
    params: perp::instruction::UpdateGlobal,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &params.data(),
        perp::accounts::UpdateGlobal {
            authority,
            global_config,
        }
        .to_account_metas(None),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn make_initialize_market_ix(
    program_id: Pubkey,
    payer: Pubkey,
    global_config: Pubkey,
    market: Pubkey,
    vault: Pubkey,
    insurance_fund_vault: Pubkey,
    lp_pool: Pubkey,
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
            insurance_fund_vault,
            lp_pool,
            oracle,
            usdc_mint,
            token_program: anchor_spl::token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn make_initialize_pool_ix(
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
pub fn make_deposit_ix(
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
pub fn make_open_position_ix(
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
pub fn make_close_position_ix(
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

#[allow(clippy::too_many_arguments)]
pub fn make_liquidate_ix(
    program_id: Pubkey,
    liquidator: Pubkey,
    trader: Pubkey,
    price_update: Pubkey,
    position: Pubkey,
    market_vault: Pubkey,
    insurance_fund_vault: Pubkey,
    market: Pubkey,
    liquidator_usdc_ata: Pubkey,
    usdc_mint: Pubkey,
) -> Instruction {
    Instruction::new_with_bytes(
        program_id,
        &perp::instruction::Liquidate {}.data(),
        perp::accounts::Liquidate {
            liquidator,
            trader,
            price_update,
            position,
            market_vault,
            insurance_fund_vault,
            market,
            liquidator_usdc_ata,
            usdc_mint,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    )
}

// --- fabricated Pyth price account -----------------------------------------

/// Builds a `PriceUpdateV2` account by hand and injects it into LiteSVM via
/// `set_account`, so a position ix's `get_price_no_older_than` call succeeds
/// without running the real Pyth receiver program on-chain. Uses the SVM's own
/// current clock as `publish_time` so the 30s freshness check always passes.
pub fn fabricate_price_update(svm: &mut LiteSVM, price: i64) -> Pubkey {
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
