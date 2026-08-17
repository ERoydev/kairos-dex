pub mod alliases;
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK");

#[program]
pub mod perp {
    use crate::initialize_market::_initialize_market;
    use crate::close_position::_close_position;

use super::*;

    pub fn initialize_market(
        ctx: Context<InitializeMarket>,
        symbol: [u8; 16],
        config: SMParams,
    ) -> Result<()> {
        _initialize_market(ctx, symbol, config)
    }

    pub fn initialize_global(
        ctx: Context<InitializeGlobal>,
        fee_receiver: Pubkey,
        max_markets: u16,
    ) -> Result<()> {
        _initialize_global(ctx, fee_receiver, max_markets)
    }

    pub fn update_global(ctx: Context<UpdateGlobal>, params: UpdateGlobalParams) -> Result<()> {
        _update_global(ctx, params)
    }

    pub fn open_position(ctx: Context<OpenPosition>, o_params: OpenPositionParams) -> Result<()> {
        _open_position(ctx, o_params)
    }

    pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
        _close_position(ctx)
    }

    pub fn market_pause(ctx: Context<MarketPause>, is_active: bool) -> Result<()> {
        _market_pause(ctx, is_active)
    }
}
