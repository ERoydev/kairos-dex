// `accounts` isn't glob-exported here: some nested types (e.g. `TvlScaledCaps`) are shared
// by both events and accounts in the IDL, so re-exporting both flattened would collide.
// Reach it via `crate::parser::accounts::*` instead. Same reasoning applies to `lp`, which
// has its own events/accounts/decoder for the liquidity-pool program — reach it via
// `crate::parser::lp::*`.
pub mod accounts;
pub mod decoder;
pub mod events;
pub mod lp;
pub mod notification;

pub use decoder::*;
pub use events::*;
