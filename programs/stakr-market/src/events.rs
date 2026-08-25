use anchor_lang::prelude::*;

#[event]
pub struct MarketCreated {
    pub market: Pubkey,
    pub authority: Pubkey,
    pub description: String,
    pub rules: String,
    pub created_at: i64,
    pub deadline: i64,
    pub b: u64,
    pub mint: Pubkey,
    pub avatar_url: String,
}

#[event]
pub struct SharesPurchased {
    pub market: Pubkey,
    pub user: Pubkey,
    pub outcome: u8,
    pub shares: u64,
    pub cost: u64,
}

#[event]
pub struct MarketResolved {
    pub market: Pubkey,
    pub winning_outcome: u8,
}

#[event]
pub struct WinningsDisbursed {
    pub market: Pubkey,
    pub position: Pubkey,
    pub user: Pubkey,
    pub payout: u64,
}

#[event]
pub struct ResidualWithdrawn {
    pub market: Pubkey,
    pub authority: Pubkey,
    pub amount: u64,
}

#[event]
pub struct MarketClosed {
    pub market: Pubkey,
    pub authority: Pubkey,
}
