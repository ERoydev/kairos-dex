pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;
pub mod events;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;
pub use error::*;

declare_id!("pCbREQ2zyCWGZ61CCsySAiMmWkKvcQreviF7yL3NP4B");

#[program]
pub mod perp {
    use super::*;

    pub fn initialize_market(ctx: Context<InitializeMarket>, symbol: [u8; 16], config: MConfig) -> Result<()> {
        crate::instructions::_initialize_market(ctx, symbol, config)
    }

    pub fn initialize_global(ctx: Context<InitializeGlobal>, fee_receiver: Pubkey, max_markets: u16) -> Result<()> {
        crate::instructions::initialize_global::_initialize_global(ctx, fee_receiver, max_markets)
    }

    pub fn update_global(ctx: Context<UpdateGlobal>, params: UpdateGlobalParams) -> Result<()> {
        crate::instructions::update_global::_update_global(ctx, params)
    }

    pub fn open_position(ctx: Context<OpenPosition>) -> Result<()> {
        crate::instructions::open_position::_open_position(ctx)
    }

}
