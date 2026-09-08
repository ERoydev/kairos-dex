

pub enum Job {
    FundingTick(String), // market id
    LiquidationCheck(String), // position id
}