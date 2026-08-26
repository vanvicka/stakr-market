use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::associated_token::AssociatedToken;
// Mint/TokenAccount come from the *legacy* anchor_spl::token module because
// only those implement `anchor_lang::Owner`, which `Account<'_, T>` requires.
// The CPI calls go through anchor_spl::token_interface's TokenInterface (so
// Token2022 token_program is supported at runtime); the Mint/TokenAccount
// *layouts* are byte-identical between the two modules, so this is safe.
use anchor_spl::token::{Mint, TokenAccount};
use anchor_spl::token_interface::{
    close_account, transfer_checked, CloseAccount, TokenInterface, TransferChecked as SplTransfer,
};
use crate::errors::PredictionMarketError;
use crate::events::*;
use crate::state::*;
use crate::math;

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
    let clock = Clock::get()?;
    require!(
        created_at.abs_diff(clock.unix_timestamp) <= 300,
        PredictionMarketError::InvalidTimestamp
    );
    require!(deadline > created_at, PredictionMarketError::DeadlineInPast);
    require!(!description.is_empty(), PredictionMarketError::EmptyDescription);
    require!(description.len() <= MAX_DESCRIPTION_LEN, PredictionMarketError::DescriptionTooLong);
    require!(!rules.is_empty(), PredictionMarketError::EmptyRules);
    require!(rules.len() <= MAX_RULES_LEN, PredictionMarketError::RulesTooLong);
    require!(b > 0, PredictionMarketError::InvalidLiquidity);
    require!(!category.is_empty(), PredictionMarketError::InvalidCategory);
    require!(category.len() <= MAX_CATEGORY_LEN, PredictionMarketError::InvalidCategory);
    require!(avatar_url.len() <= MAX_AVATAR_URL_LEN, PredictionMarketError::InvalidAvatarUrl);

    let market = &mut ctx.accounts.market;
    market.authority = ctx.accounts.authority.key();
    market.description = description;
    market.rules = rules;
    market.created_at = created_at;
    market.deadline = deadline;
    market.resolved = false;
    market.resolved_at = 0;
    market.winning_outcome = WINNING_OUTCOME_NONE;
    market.q_yes = 0;
    market.q_no = 0;
    market.b = b;
    market.pool_balance = 0;
    market.mint = ctx.accounts.mint.key();
    market.mint_decimals = ctx.accounts.mint.decimals;
    market.market_bump = ctx.bumps.market;
    market.category = category;
    market.avatar_url = avatar_url;

    emit!(MarketCreated {
        market: market.key(),
        authority: market.authority,
        description: market.description.clone(),
        rules: market.rules.clone(),
        created_at: market.created_at,
        deadline,
        b,
        mint: market.mint,
        avatar_url: market.avatar_url.clone(),
    });

    Ok(())
}

pub fn buy_shares(ctx: Context<BuyShares>, nonce: i64, outcome: u8, shares: u64) -> Result<()> {
    require!(outcome == 0 || outcome == 1, PredictionMarketError::InvalidOutcome);
    require!(shares > 0, PredictionMarketError::ZeroShares);
    let clock = Clock::get()?;
    require!(
        nonce.abs_diff(clock.unix_timestamp) <= 300,
        PredictionMarketError::InvalidTimestamp
    );
    let market = &mut ctx.accounts.market;
    require!(!market.resolved, PredictionMarketError::MarketAlreadyResolved);
    require!(clock.unix_timestamp < market.deadline, PredictionMarketError::DeadlinePassed);
    let cost = math::lmsr_buy_cost(market.q_yes, market.q_no, outcome, shares, market.b)
        .ok_or(PredictionMarketError::Overflow)?;

    let cpi_accounts = SplTransfer {
        from: ctx.accounts.user_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.treasury.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.key(),
        cpi_accounts,
    );
    transfer_checked(cpi_ctx, cost, market.mint_decimals)?;

    let q = if outcome == 0 { &mut market.q_yes } else { &mut market.q_no };
    *q = q.checked_add(shares).ok_or(PredictionMarketError::Overflow)?;

    market.pool_balance = market.pool_balance.checked_add(cost)
        .ok_or(PredictionMarketError::Overflow)?;

    let position = &mut ctx.accounts.position;
    if position.user == Pubkey::default() {
        position.user = ctx.accounts.user.key();
        position.market = market.key();
    }

    let p = if outcome == 0 { &mut position.yes_shares } else { &mut position.no_shares };
    *p = p.checked_add(shares).ok_or(PredictionMarketError::Overflow)?;

    if outcome == 0 {
        position.yes_cost = position.yes_cost
            .checked_add(cost)
            .ok_or(PredictionMarketError::Overflow)?;
    } else {
        position.no_cost = position.no_cost
            .checked_add(cost)
            .ok_or(PredictionMarketError::Overflow)?;
    }

    position.cost = position.cost.checked_add(cost)
        .ok_or(PredictionMarketError::Overflow)?;

    emit!(SharesPurchased {
        market: market.key(),
        user: ctx.accounts.user.key(),
        outcome,
        shares,
        cost,
    });

    Ok(())
}

pub fn resolve_market(ctx: Context<ResolveMarket>, winning_outcome: u8) -> Result<()> {
    // Centralization risk: the market authority selects the winning outcome
    // off-chain and asserts it here — there is no on-chain oracle or tally of
    // user positions. The `rules` string (set at creation) is the only human
    // recourse for disputing a resolution. To remove this trust assumption
    // integrate an oracle (e.g. Switchboard) or a committee/multisig on the
    // authority before mainnet deployment.
    require!(winning_outcome == 0 || winning_outcome == 1, PredictionMarketError::InvalidOutcome);
    let clock = Clock::get()?;
    let market = &mut ctx.accounts.market;
    require!(!market.resolved, PredictionMarketError::MarketAlreadyResolved);
    require!(clock.unix_timestamp >= market.deadline, PredictionMarketError::DeadlineNotPassed);
    require!(
        ctx.accounts.authority.key() == market.authority,
        PredictionMarketError::Unauthorized
    );
    market.resolved = true;
    market.resolved_at = clock.unix_timestamp;
    market.winning_outcome = winning_outcome;

    emit!(MarketResolved {
        market: market.key(),
        winning_outcome,
    });

    Ok(())
}

pub fn disburse<'info>(ctx: Context<'info, Disburse<'info>>) -> Result<()> {
    let market = &ctx.accounts.market;
    require!(market.resolved, PredictionMarketError::MarketNotResolved);

    // Divergence guard: `pool_balance` mirrors the SPL treasury balance and is
    // the per-batch payout basis. If they ever disagree, payouts would be
    // computed against a stale pool — fail fast rather than under/overpay.
    require!(
        market.pool_balance == ctx.accounts.treasury.amount,
        PredictionMarketError::InsufficientTreasury
    );

    let winning_outcome = market.winning_outcome;
    let total_winning_shares = if winning_outcome == 0 {
        market.q_yes
    } else {
        market.q_no
    };
    require!(total_winning_shares > 0, PredictionMarketError::WrongOutcome);

    let remaining = ctx.remaining_accounts;
    let payer_info = ctx.accounts.payer.to_account_info();
    let mut to_pay: Vec<(AccountInfo, AccountInfo, AccountInfo, u64)> = Vec::new();
    let mut batch_total: u128 = 0;
    let mut batch_winning_shares: u128 = 0;

    // Reject duplicate accounts up front: a Position appearing twice would be
    // pushed into `to_pay` twice and double-paid before pass-2 close runs
    // (close happens after all of pass 1, so the in-loop deserializability
    // guard does not protect us intra-batch). Duplicate ATA/SOL accounts are
    // also rejected for simplicity and to rule out ambiguous close moves.
    {
        let mut seen = std::collections::HashSet::new();
        for acc in remaining.iter() {
            if !seen.insert(acc.key()) {
                return Err(PredictionMarketError::InvalidPosition.into());
            }
        }
    }

    // Pass 1: validate every winning position, sum the batch payout, and
    // locate each winner's USDC ATA and SOL account among the remaining
    // accounts (shared across positions owned by the same user). If a winner
    // is the fee-paying `payer`, their SOL account is used directly.
    for acc in remaining.iter() {
        if acc.owner != ctx.program_id {
            continue; // ATA or user SOL account
        }

        let position = Position::try_deserialize(&mut &acc.data.borrow()[..])?;
        require!(
            position.market == market.key(),
            PredictionMarketError::InvalidPosition
        );

        let winning_shares = if winning_outcome == 0 {
            position.yes_shares
        } else {
            position.no_shares
        };
        require!(winning_shares > 0, PredictionMarketError::WrongOutcome);

        let payout = (winning_shares as u128)
            .checked_mul(market.pool_balance as u128)
            .and_then(|v| v.checked_div(total_winning_shares as u128))
            .ok_or(PredictionMarketError::Overflow)? as u64;

        let ata_address =
            get_associated_token_address(&position.user, &ctx.accounts.mint.key());
        let ata_acc = remaining
            .iter()
            .find(|a| a.key() == ata_address)
            .ok_or(PredictionMarketError::InvalidPosition)?
            .clone();
        {
            // Validate the ATA is a token account for `mint` owned by the
            // position holder. SPL token Account layout: mint at 0, owner at 32.
            let ata_data = ata_acc.data.borrow();
            if ata_data.len() < 64 {
                return Err(PredictionMarketError::InvalidPosition.into());
            }
            let ata_mint = Pubkey::from(<[u8; 32]>::try_from(&ata_data[0..32]).unwrap());
            let ata_owner = Pubkey::from(<[u8; 32]>::try_from(&ata_data[32..64]).unwrap());
            require!(
                *ata_acc.owner == ctx.accounts.token_program.key()
                    && ata_owner == position.user
                    && ata_mint == market.mint,
                PredictionMarketError::InvalidPosition
            );
        }

        let user_acc = remaining
            .iter()
            .find(|a| a.key() == position.user)
            .cloned()
            .or_else(|| {
                if ctx.accounts.payer.key() == position.user {
                    Some(payer_info.clone())
                } else {
                    None
                }
            })
            .ok_or(PredictionMarketError::InvalidPosition)?;

        batch_total = batch_total
            .checked_add(payout as u128)
            .ok_or(PredictionMarketError::Overflow)?;
        batch_winning_shares = batch_winning_shares
            .checked_add(winning_shares as u128)
            .ok_or(PredictionMarketError::Overflow)?;
        require!(
            batch_total <= ctx.accounts.treasury.amount as u128,
            PredictionMarketError::InsufficientTreasury
        );

        to_pay.push((acc.clone(), user_acc, ata_acc, payout));
    }

    // Pass 2: pay each winner and close the position to recover its rent.
    //
    // Why a hand-rolled close instead of anchor_spl::token_interface::Close?
    // The Position is a program-owned PDA, not a TOKEN account; the SPL
    // `close_account` CPI only works on Token/Token2022 accounts. So we
    // reproduce Anchor's close semantics manually: move the account's
    // lamports to the winner, zero its data, and reassign it to the system
    // program — making it ineligible for `Position::try_deserialize` on any
    // future call. That "closed ⇒ not deserializable" property IS what
    // prevents double-disbursement (replaced the former `claimed` bool).
    for (position_acc, user_acc, ata_acc, payout) in to_pay {
        let created_at = market.created_at.to_le_bytes();
        let seeds = &[
            b"market",
            market.authority.as_ref(),
            created_at.as_ref(),
            &[market.market_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_accounts = SplTransfer {
            from: ctx.accounts.treasury.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ata_acc.to_account_info(),
            authority: ctx.accounts.market.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            cpi_accounts,
            signer_seeds,
        );
        transfer_checked(cpi_ctx, payout, market.mint_decimals)?;

        let pos_lamports = position_acc.lamports();
        {
            let mut user_lamports = user_acc.try_borrow_mut_lamports()?;
            let new_lamports = (*user_lamports)
                .checked_add(pos_lamports)
                .ok_or(PredictionMarketError::Overflow)?;
            **user_lamports = new_lamports;
        }
        {
            let mut pos_lamports = position_acc.try_borrow_mut_lamports()?;
            **pos_lamports = 0;
        }
        {
            let mut data = position_acc.try_borrow_mut_data()?;
            data.fill(0);
        }
        position_acc.assign(&anchor_lang::system_program::ID);

        emit!(WinningsDisbursed {
            market: market.key(),
            position: position_acc.key(),
            user: user_acc.key(),
            payout,
        });
    }

    // Decrement the cached `pool_balance` AND the winning-shares counter by
    // the amounts actually paid this batch so the next batch's per-winner
    // payout (winning_shares * pool_balance / total_winning_shares) divides
    // the *remaining* pool by the *remaining* winning shares — otherwise a
    // later batch would compute against the original (now-reduced) pool and
    // either underpay the winners (shares are fixed but pool shrank) or stall
    // on `InsufficientTreasury`, stranding later winners until the residual
    // window lets the authority sweep their winnings. Anchor re-serializes
    // the mut Market PDA at ix end.
    if batch_total > 0 {
        let paid = u64::try_from(batch_total).map_err(|_| PredictionMarketError::Overflow)?;
        ctx.accounts.market.pool_balance = ctx
            .accounts
            .market
            .pool_balance
            .checked_sub(paid)
            .ok_or(PredictionMarketError::InsufficientTreasury)?;

        let paid_shares =
            u64::try_from(batch_winning_shares).map_err(|_| PredictionMarketError::Overflow)?;
        let winning_counter = if winning_outcome == 0 {
            &mut ctx.accounts.market.q_yes
        } else {
            &mut ctx.accounts.market.q_no
        };
        *winning_counter = winning_counter
            .checked_sub(paid_shares)
            .ok_or(PredictionMarketError::Overflow)?;
    }

    Ok(())
}

pub fn withdraw_residual(ctx: Context<WithdrawResidual>) -> Result<()> {
    let market = &ctx.accounts.market;
    require!(market.resolved, PredictionMarketError::MarketNotResolved);

    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp > market.resolved_at + CLAIM_WINDOW_SECONDS,
        PredictionMarketError::ResidualNotAvailable
    );

    require!(
        ctx.accounts.authority.key() == market.authority,
        PredictionMarketError::Unauthorized
    );

    let balance = ctx.accounts.treasury.amount;
    require!(balance > 0, PredictionMarketError::ResidualNotAvailable);

    let created_at = market.created_at.to_le_bytes();
    let seeds = &[
        b"market",
        market.authority.as_ref(),
        created_at.as_ref(),
        &[market.market_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let cpi_accounts = SplTransfer {
        from: ctx.accounts.treasury.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.authority_token_account.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        cpi_accounts,
        signer_seeds,
    );
    transfer_checked(cpi_ctx, balance, market.mint_decimals)?;

    let close_accounts = CloseAccount {
        account: ctx.accounts.treasury.to_account_info(),
        destination: ctx.accounts.authority_token_account.to_account_info(),
        authority: ctx.accounts.market.to_account_info(),
    };
    let close_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        close_accounts,
        signer_seeds,
    );
    close_account(close_ctx)?;

    // Close the Market account now that the claim window has elapsed, its
    // winners have been paid (or let their claims lapse) and the residual has
    // been swept. Reclaim its rent back to the authority and reassign it to
    // the system program so the account can never be used again. Same manual
    // close semantics as `disburse` for positions.
    let market_acc = ctx.accounts.market.to_account_info();
    let market_lamports = market_acc.lamports();
    {
        let mut authority_lamports = ctx.accounts.authority.try_borrow_mut_lamports()?;
        let new_lamports = (*authority_lamports)
            .checked_add(market_lamports)
            .ok_or(PredictionMarketError::Overflow)?;
        **authority_lamports = new_lamports;
    }
    {
        let mut market_lamports = market_acc.try_borrow_mut_lamports()?;
        **market_lamports = 0;
    }
    {
        let mut data = market_acc.try_borrow_mut_data()?;
        data.fill(0);
    }
    market_acc.assign(&anchor_lang::system_program::ID);

    emit!(ResidualWithdrawn {
        market: market.key(),
        authority: ctx.accounts.authority.key(),
        amount: balance,
    });
    emit!(MarketClosed {
        market: market.key(),
        authority: ctx.accounts.authority.key(),
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(created_at: i64, description: String, rules: String, deadline: i64, b: u64, category: String, avatar_url: String)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + Market::INIT_SPACE,
        seeds = [
            b"market",
            authority.key().as_ref(),
            created_at.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub market: Account<'info, Market>,
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = market,
    )]
    pub treasury: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(nonce: i64, outcome: u8, shares: u64)]
pub struct BuyShares<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [
            b"market",
            market.authority.as_ref(),
            market.created_at.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        constraint = user_token_account.mint == market.mint @ PredictionMarketError::InvalidPosition,
        constraint = user_token_account.owner == user.key() @ PredictionMarketError::InvalidPosition,
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = market.mint,
        associated_token::authority = market,
    )]
    pub treasury: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + Position::INIT_SPACE,
        seeds = [
            b"position",
            user.key().as_ref(),
            market.key().as_ref(),
            nonce.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub position: Account<'info, Position>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(winning_outcome: u8)]
pub struct ResolveMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [
            b"market",
            market.authority.as_ref(),
            market.created_at.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub market: Account<'info, Market>,
}

#[derive(Accounts)]
pub struct WithdrawResidual<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [
            b"market",
            market.authority.as_ref(),
            market.created_at.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        constraint = authority_token_account.mint == market.mint @ PredictionMarketError::InvalidPosition,
        constraint = authority_token_account.owner == authority.key() @ PredictionMarketError::InvalidPosition,
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = market.mint,
        associated_token::authority = market,
    )]
    pub treasury: Account<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Disburse<'info> {
    /// Pays the transaction fee; anyone may trigger payout batches. Writable
    /// because when the payer is also a winner their SOL account receives the
    /// closed position's rent via the payer-fallback path.
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [
            b"market",
            market.authority.as_ref(),
            market.created_at.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub market: Account<'info, Market>,
    #[account(
        mut,
        associated_token::mint = market.mint,
        associated_token::authority = market,
    )]
    pub treasury: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    // remaining_accounts: per winner the Position (writable), the user's
    // USDC ATA (writable), and the user's SOL account (writable, receives
    // the position's rent on close). Shared ATA/SOL accounts appear once.
}
