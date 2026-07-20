pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("4ZuUyESpBpUfAYperMNg9h6uzKMCNjfL47RsNfD3TnLL");

#[program]
pub mod liquidity_pool {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>) -> Result<()> {
        _initialize_pool(ctx)
    }
}
