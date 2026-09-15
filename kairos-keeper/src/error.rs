use derive_more::{Display, From};
use solana_sdk::pubkey::ParsePubkeyError;

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

    // -- Externals
    #[from]
    Io(std::io::Error),
    #[from]
    Rpc(rpc::RpcError),
}

// region:    --- Custom

impl std::error::Error for Error {}

// endregion: --- Custom
