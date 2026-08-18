use crate::alliases::MicroUsdc;

pub struct Skew<'a> {
    curr_oi_long: &'a MicroUsdc,
    curr_oi_short: &'a MicroUsdc,
    notional: &'a MicroUsdc,
    is_long: bool,
}

impl<'a> Skew<'a> {
    pub fn new(
        curr_oi_long: &'a MicroUsdc,
        curr_oi_short: &'a MicroUsdc,
        notional: &'a MicroUsdc,
        is_long: bool,
    ) -> Skew<'a> {
        Skew {
            curr_oi_long,
            curr_oi_short,
            notional,
            is_long,
        }
    }

    /// Gives me the difference between total long and short positions in the Market, that this trade is going to create
    pub fn projected_skew(&self) -> MicroUsdc {
        if self.is_long {
            (self.curr_oi_long + self.notional).abs_diff(*self.curr_oi_short)
        } else {
            self.curr_oi_long
                .abs_diff(self.curr_oi_short + self.notional)
        }
    }

    /// Calculate if skew gets worse with new position that is going to result in charging extra fee
    pub fn worsens_skew(&self) -> bool {
        let curr_skew = self.get_skew();
        let projected_skew = self.projected_skew();

        projected_skew > curr_skew
    }

    fn get_skew(&self) -> MicroUsdc {
        self.curr_oi_long.abs_diff(*self.curr_oi_short)
    }
}
