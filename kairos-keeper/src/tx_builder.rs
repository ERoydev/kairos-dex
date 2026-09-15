use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use perp::accounts as perp_accounts;
use perp::instruction as perp_ix;
use perp::{INSURANCE_FUND_VAULT, MARKET_VAULT};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

use crate::error::{Error, Result};

/// `perp` pins anchor-lang 1.1.2, which resolves an older `solana-pubkey` (3.0.0) than the
/// `solana-sdk` 4.1.0 this crate uses for RPC (solana-pubkey 4.3.0). Both `Pubkey` types are
/// 32-byte wrappers but not the same Rust type, so every value crossing the anchor boundary is
/// converted at the byte level via these two helpers.
fn to_anchor_pubkey(p: Pubkey) -> anchor_lang::prelude::Pubkey {
    anchor_lang::prelude::Pubkey::from(p.to_bytes())
}

fn to_sdk_pubkey(p: anchor_lang::prelude::Pubkey) -> Pubkey {
    Pubkey::from(p.to_bytes())
}

fn to_sdk_account_meta(m: anchor_lang::solana_program::instruction::AccountMeta) -> AccountMeta {
    AccountMeta {
        pubkey: to_sdk_pubkey(m.pubkey),
        is_signer: m.is_signer,
        is_writable: m.is_writable,
    }
}

/// Builds the permissionless `update_funding` instruction for a given market.
pub fn build_update_funding_ix(signer: Pubkey, market: Pubkey) -> Instruction {
    let accounts = perp_accounts::UpdateFunding {
        signer: to_anchor_pubkey(signer),
        market: to_anchor_pubkey(market),
        system_program: anchor_lang::solana_program::system_program::ID,
    }
    .to_account_metas(None)
    .into_iter()
    .map(to_sdk_account_meta)
    .collect();

    Instruction {
        program_id: to_sdk_pubkey(perp::ID),
        accounts,
        data: perp_ix::UpdateFunding {}.data(),
    }
}

/// Fetches an account and decodes it with the target program's own `AccountDeserialize` impl
/// (rather than `rpc::AnchorAccount`, which `perp`'s account types don't implement — they're a
/// foreign type from a foreign trait's perspective here, so bridging them isn't possible without
/// wrapping types; anchor-lang's own trait, which `perp` already implements them with, sidesteps
/// that).
async fn fetch_anchor_account<T: AccountDeserialize>(
    rpc_client: &rpc::RpcClient,
    pubkey: &Pubkey,
) -> Result<T> {
    let data = rpc_client
        .get_account_data(pubkey)
        .await?
        .ok_or(Error::AccountNotFound(*pubkey))?;

    T::try_deserialize(&mut data.as_slice()).map_err(|e| Error::AccountDecode(e.to_string()))
}

// TODO
/// Builds the permissionless `liquidate` instruction for a given position. Unlike
/// `build_update_funding_ix`, this needs on-chain reads first: the position only carries
/// `(trader, market)`, and the market only carries the oracle account — every other account
/// (vaults, mint, liquidator's ATA) is derived or supplied from there.
pub async fn build_liquidate_ix(
    rpc_client: &rpc::RpcClient,
    liquidator: Pubkey,
    position_pubkey: Pubkey,
    usdc_mint: Pubkey,
) -> Result<Instruction> {
    let position: perp::position::Position =
        fetch_anchor_account(rpc_client, &position_pubkey).await?;
    let market_pubkey = to_sdk_pubkey(position.market);

    let market: perp::syntetic_market::SynteticMarket =
        fetch_anchor_account(rpc_client, &market_pubkey).await?;

    let program_id = to_sdk_pubkey(perp::ID);
    let (market_vault, _) =
        Pubkey::find_program_address(&[MARKET_VAULT, market_pubkey.as_ref()], &program_id);
    let (insurance_fund_vault, _) =
        Pubkey::find_program_address(&[INSURANCE_FUND_VAULT, market_pubkey.as_ref()], &program_id);

    let liquidator_anchor = to_anchor_pubkey(liquidator);
    let usdc_mint_anchor = to_anchor_pubkey(usdc_mint);
    let liquidator_usdc_ata = anchor_spl::associated_token::get_associated_token_address(
        &liquidator_anchor,
        &usdc_mint_anchor,
    );

    let accounts = perp_accounts::Liquidate {
        liquidator: liquidator_anchor,
        trader: position.owner,
        price_update: market.oracle,
        position: to_anchor_pubkey(position_pubkey),
        market_vault: to_anchor_pubkey(market_vault),
        insurance_fund_vault: to_anchor_pubkey(insurance_fund_vault),
        market: position.market,
        liquidator_usdc_ata,
        usdc_mint: usdc_mint_anchor,
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None)
    .into_iter()
    .map(to_sdk_account_meta)
    .collect();

    Ok(Instruction {
        program_id,
        accounts,
        data: perp_ix::Liquidate {}.data(),
    })
}
