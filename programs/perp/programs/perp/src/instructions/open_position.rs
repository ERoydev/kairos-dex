use anchor_lang::prelude::*;

use crate::{
    position::{Position, PositionType},
    syntetic_market::SynteticMarket,
    POSITION_SEED,
};

pub fn _open_position(_ctx: Context<OpenPosition>, _o_params: OpenPositionParams) -> Result<()> {
    // TODO: Validations

    // Read oracle price

    // calculate fee

    // deduct fee from margin

    // Handle fee and margin stores TODO

    // Create Position

    // Update Market open interest
    // size = margin * leverage
    // match side {
    //     Side::Long => market.open_interest_long += size,
    //     Side::Short => market.open_interest_short += size,
    // }

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
    pub market: Account<'info, SynteticMarket>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OpenPositionParams {
    pub leverage: u64,
    pub margin: u64,
    pub take_profit: u64,
    pub stop_loss: u64,
    pub position_type: PositionType,
}
