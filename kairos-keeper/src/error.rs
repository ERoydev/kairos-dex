use derive_more::{Display, From};
use solana_sdk::pubkey::ParsePubkeyError;
use solana_sdk::pubkey::Pubkey;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Display, From)]
#[display("{self:?}")]
pub enum Error {
    // -- fs (i do organization by module)
    FsEmptyFolder,
    // #[from]
    // Fs(fs::Error),

    // -- Task / tx building
    #[from]
    InvalidMarketId(ParsePubkeyError),
    /// An account a liquidation tx depends on (position, market, ...) doesn't exist on-chain.
    AccountNotFound(Pubkey),
    /// Raw account bytes didn't decode as the Anchor account type we expected.
    AccountDecode(String),

    // -- Externals
    #[from]
    Io(std::io::Error),
    #[from]
    Rpc(rpc::RpcError),
}

// region:    --- Custom

impl std::error::Error for Error {}

// endregion: --- Custom
