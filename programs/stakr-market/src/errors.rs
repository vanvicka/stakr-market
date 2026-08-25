use anchor_lang::prelude::*;

#[error_code]
pub enum PredictionMarketError {
    #[msg("Market is already resolved")]
    MarketAlreadyResolved,
    #[msg("Market has not been resolved yet")]
    MarketNotResolved,
    #[msg("Deadline has not passed yet")]
    DeadlineNotPassed,
    #[msg("Deadline must be in the future")]
    DeadlineInPast,
    #[msg("Deadline has already passed")]
    DeadlinePassed,
    #[msg("Invalid outcome. Must be 0 or 1")]
    InvalidOutcome,
    #[msg("Description cannot be empty")]
    EmptyDescription,
    #[msg("Rules cannot be empty")]
    EmptyRules,
    #[msg("Share count must be greater than zero")]
    ZeroShares,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Liquidity parameter b must be greater than zero")]
    InvalidLiquidity,
    #[msg("You did not bet on this outcome")]
    WrongOutcome,
    #[msg("Description too long")]
    DescriptionTooLong,
    #[msg("Rules too long")]
    RulesTooLong,
    #[msg("Residual withdrawal is not yet available")]
    ResidualNotAvailable,
    #[msg("Treasury balance is not enough for this payout batch")]
    InsufficientTreasury,
    #[msg("Position does not belong to this market")]
    InvalidPosition,
    #[msg("Invalid market category")]
    InvalidCategory,
    #[msg("Avatar URL too long")]
    InvalidAvatarUrl,
    #[msg("Timestamp is outside the accepted window")]
    InvalidTimestamp,
}