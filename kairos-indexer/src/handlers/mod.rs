pub mod funding;
pub mod lp;
pub mod markets;
pub mod positions;

use anchor_lang::AccountDeserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

const ACCOUNT_FETCH_RETRIES: u32 = 3;
const ACCOUNT_FETCH_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// Fetches and decodes an on-chain account for a field an event doesn't itself carry. Returns
/// `None` (logging why) if there's no RPC client, the account fetch/decode fails, or the
/// account still isn't visible after a few short retries — callers fall back to a placeholder
/// rather than treat this as fatal.
///
/// The retry exists because the WS `logsSubscribe` notification and a plain `getAccountInfo`
/// call don't always see the same state at the same instant — e.g. Helius may serve them from
/// different backend nodes — so the account this event just created can briefly 404 right
/// after we're notified about the transaction that created it.
pub(crate) async fn fetch_account<T: AccountDeserialize>(
    rpc: Option<&RpcClient>,
    pubkey: &Pubkey,
    context: &str,
) -> Option<T> {
    let rpc = rpc?;

    for attempt in 1..=ACCOUNT_FETCH_RETRIES {
        match rpc
            .get_account_with_commitment(pubkey, CommitmentConfig::finalized())
            .await
        {
            Ok(response) => match response.value {
                Some(account) => {
                    return match T::try_deserialize(&mut account.data.as_slice()) {
                        Ok(decoded) => Some(decoded),

                        Err(err) => {
                            eprintln!("{context}: failed to decode account {pubkey}: {err}");
                            None
                        }
                    };
                }
                None if attempt < ACCOUNT_FETCH_RETRIES => {
                    tokio::time::sleep(ACCOUNT_FETCH_RETRY_DELAY).await;
                }
                None => {
                    eprintln!(
                        "{context}: account {pubkey} still not found after {attempt} attempts"
                    )
                }
            },
            Err(err) => {
                eprintln!("{context}: failed to fetch account {pubkey}: {err}");
                return None;
            }
        }
    }

    None
}
