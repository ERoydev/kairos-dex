use derive_more::{Display, From};
use solana_client::client_error::ClientError;
use solana_sdk::{pubkey::ParsePubkeyError, signature::ParseSignatureError};

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Display, From)]
#[display("{self:?}")]
pub enum Error {
    // -- fs (i do organization by module)
    FsEmptyFolder,
    // #[from]
    // Fs(fs::Error),

    // -- Customs
    EmptySignatures,

    // -- Externals
    #[from]
    Io(std::io::Error),
    #[from]
    Rpc(ClientError), // RpcClient Error
    #[from]
    ParsePubkey(ParsePubkeyError),
    #[from]
    ParseSignatureError(ParseSignatureError),
    #[from]
    Json(serde_json::Error),
}

// region:    --- Custom

impl std::error::Error for Error {}

// endregion: --- Custom
