use anchor_lang::prelude::*;

// Category is stored as a human-readable string (e.g. "Sports", "Lottery").
pub const MAX_CATEGORY_LEN: usize = 20;
pub const MAX_DESCRIPTION_LEN: usize = 100;
pub const MAX_RULES_LEN: usize = 400;
pub const MAX_AVATAR_URL_LEN: usize = 150;

/// Sentinel value stored in `Market.winning_outcome` before resolution.
/// Only `0` (yes) and `1` (no) are valid resolved outcomes; `2` means "not
/// resolved yet" and is never accepted by `resolve_market` (InvalidOutcome).
pub const WINNING_OUTCOME_NONE: u8 = 2;

/// Winners may claim for this long after resolution; after it elapses the
/// market authority may withdraw the remaining pool balance as residual.
pub const CLAIM_WINDOW_SECONDS: i64 = 30 * 86400;

#[account]
#[derive(InitSpace)]
pub struct Market {
    pub authority: Pubkey,
    #[max_len(MAX_DESCRIPTION_LEN)]
    pub description: String,
    #[max_len(MAX_RULES_LEN)]
    pub rules: String,
    pub created_at: i64,
    pub deadline: i64,
    pub resolved: bool,
    pub resolved_at: i64,
    pub winning_outcome: u8,
    pub q_yes: u64,
    pub q_no: u64,
    pub b: u64,
    pub pool_balance: u64,
    pub mint: Pubkey,
    pub mint_decimals: u8,
    pub market_bump: u8,
    #[max_len(MAX_CATEGORY_LEN)]
    pub category: String,
    #[max_len(MAX_AVATAR_URL_LEN)]
    pub avatar_url: String,
}

#[account]
#[derive(InitSpace)]
pub struct Position {
    pub user: Pubkey,
    pub market: Pubkey,
    pub yes_shares: u64,
    pub no_shares: u64,
    pub yes_cost: u64,
    pub no_cost: u64,
    pub cost: u64,
    // NOTE: a paid position is *closed* by `disburse`: its data is zeroed, its
    // lamports are moved to the winner, and the account is reassigned to the
    // system program. Therefore "this Position still exists and
    // deserializes" IS the invariant "not yet paid" — re-disbursement is
    // impossible because a closed account fails `try_deserialize`.
}
