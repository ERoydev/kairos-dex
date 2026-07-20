use anchor_lang::prelude::*;

#[account]
pub struct Pool {
    // --- Versioning / upgradeability ---
    pub version: u8,              // for future migrations
    pub bump: u8,                 // PDA bump seed

    // --- Admin ---
    pub authority: Pubkey,        // who can update config

    // --- Asset config ---
    pub usdc_mint: Pubkey,        // which token the pool holds (USDC)
    pub usdc_vault: Pubkey,       // SPL token account holding the USDC
    pub lp_mint: Pubkey,          // SPL mint for LP share tokens

    // --- Accounting ---
    pub total_assets: u64,        // total USDC in the pool
    pub total_shares: u64,        // total LP shares outstanding

    // --- Reserved for future fields ---
    pub _reserved: [u8; 128],
}

impl Pool {
    pub const SIZE: usize =
        std::mem::size_of::<u8>() +
        std::mem::size_of::<u8>() +
        std::mem::size_of::<Pubkey>() +
        std::mem::size_of::<Pubkey>() +
        std::mem::size_of::<Pubkey>() +
        std::mem::size_of::<Pubkey>() +
        std::mem::size_of::<u64>() +
        std::mem::size_of::<u64>() +
        128;
}

/*
Per-user shares → not needed. SPL token accounts already track that.
Perp program pubkey → will need this later for CPI access control, but you can add it via the reserved space when you get there.
Fee config → not needed for v1. Add later.
Reserve factor / caps → risk config lives in the perp contract, not the vault.
 */
