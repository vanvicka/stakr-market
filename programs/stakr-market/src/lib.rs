use anchor_lang::prelude::*;

declare_id!("BE3ERuEVgYTN8a9XfW8wpAPg4aTuwPD69RkiAs1fDEaw");

pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

#[program]
pub mod prediction_market {
    use super::*;

    pub fn create_market(
        ctx: Context<CreateMarket>,
        created_at: i64,
        description: String,
        rules: String,
        deadline: i64,
        b: u64,
        category: String,
        avatar_url: String,
    ) -> Result<()> {
        instructions::create_market(ctx, created_at, description, rules, deadline, b, category, avatar_url)
    }

    pub fn buy_shares(ctx: Context<BuyShares>, nonce: i64, outcome: u8, shares: u64) -> Result<()> {
        instructions::buy_shares(ctx, nonce, outcome, shares)
    }

    pub fn resolve_market(ctx: Context<ResolveMarket>, winning_outcome: u8) -> Result<()> {
        instructions::resolve_market(ctx, winning_outcome)
    }

    pub fn disburse<'a>(ctx: Context<'a, Disburse<'a>>) -> Result<()> {
        instructions::disburse(ctx)
    }

    pub fn withdraw_residual(ctx: Context<WithdrawResidual>) -> Result<()> {
        instructions::withdraw_residual(ctx)
    }
}