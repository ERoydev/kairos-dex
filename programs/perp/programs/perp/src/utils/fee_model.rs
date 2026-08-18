// use anchor_lang::Result;
use anchor_lang::Result;

use crate::{alliases::MicroUsdc, syntetic_market::FeeSchedule, PerpError, PROTOCOL_FEES_BPS};

pub struct FeeModel<'a> {
    notional: &'a MicroUsdc,
    fee_schedule: &'a FeeSchedule,
    projected_skew: Option<&'a MicroUsdc>,
    max_skew: Option<&'a MicroUsdc>,
}

impl<'a> FeeModel<'a> {
    pub fn new(notional: &'a MicroUsdc, fee_schedule: &'a FeeSchedule, projected_skew: Option<&'a MicroUsdc>, max_skew: Option<&'a MicroUsdc>) -> FeeModel<'a> {
        FeeModel { notional, fee_schedule, projected_skew, max_skew}
    }

    /// fee_bps: fee in basis points (1 bps = 0.01%). e.g. 10 = 0.10%
    /// size: notional position size in USDC base units
    /// returns: fee amount in USDC base units
    pub fn calculate_base_fee(&self, fee_bps: Option<MicroUsdc>) -> Result<MicroUsdc> {
        let notional = *self.notional as u128;
        let res: u128 = match fee_bps {
            // notional * fee_bps / 10_000
            Some(v) => notional
                .checked_mul(v as u128)
                .and_then(|v| v.checked_div(10_000))
                .ok_or(PerpError::MathOverflow)?,
            None => {
                let base_fee_bps = self.fee_schedule.base_fee_bps as u128;
                notional
                    .checked_mul(base_fee_bps)
                    .and_then(|v| v.checked_div(10_000))
                    .ok_or(PerpError::MathOverflow)?
            }
        };

        Ok(res as u64)
    }
    
    /// Those are `Trade Fees`, they are charged on `open` and `close`. One-time
    /// Paid at open → already deducted from collateral before position exists.
    /// Paid at close → applied when user closes, doesn't affect liquidation trigger.
    pub fn calculate_trade_fee(&self, worsens_skew: bool) -> Result<MicroUsdc> {
        let base_fee: MicroUsdc = self.calculate_base_fee(None)?;

        let skew_fee: MicroUsdc = if worsens_skew {
            let projected_skew = *self.projected_skew.ok_or(PerpError::CannotCalculateOpenFeeOnClosePosition)? as u128;
            let max_skew = *self.max_skew.ok_or(PerpError::CannotCalculateOpenFeeOnClosePosition)? as u128;
            let skew_fee_max_bps = self.fee_schedule.skew_fee_max_bps as u128;
            
            // fee_max_bps * project skew / max_skew
            let scaled_bps = skew_fee_max_bps
                .checked_mul(projected_skew)
                .and_then(|v| v.checked_div(max_skew))
                .ok_or(PerpError::MathOverflow)? as MicroUsdc;

            self.calculate_base_fee(Some(scaled_bps))?
        } else {
            0
        };

        Ok(base_fee.checked_add(skew_fee).ok_or(PerpError::MathOverflow)?)
    }

    /// protocol_fees floors down integer divison; lp_fees takes the remained via subtraction
    pub fn calc_distributed_fees(&self, total_fees: MicroUsdc) -> Result<(MicroUsdc, MicroUsdc)> {
        let protocol_fees = (total_fees as u128)
            .checked_mul(PROTOCOL_FEES_BPS as u128)
            .ok_or(PerpError::MathOverflow)?
            .checked_div(10_000)
            .ok_or(PerpError::MathOverflow)? as u64;

        let lp_fees = total_fees
            .checked_sub(protocol_fees)
            .ok_or(PerpError::MathOverflow)?;

        Ok((protocol_fees, lp_fees))
    }

}
