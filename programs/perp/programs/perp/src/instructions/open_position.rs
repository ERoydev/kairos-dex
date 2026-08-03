use anchor_lang::prelude::*;

use crate::{POSITION_SEED, market::Market, position::Position};

pub fn _open_position(_ctx: Context<OpenPosition>) -> Result<()> {
    msg!("Openning Position");
    Ok(())
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    #[account(
        init,
        payer = trader,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, trader.key().as_ref(), market.key().as_ref()],
        bump

    )]
    pub position: Account<'info, Position>,

    #[account(mut)]
    pub market: Account<'info, Market>,

    pub system_program: Program<'info, System>,
}
