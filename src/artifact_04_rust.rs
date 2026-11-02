use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("BrkP199999999999999999999999999999999999999");

pub const DISBURSEMENT_STAGE_1_PERCENT: u64 = 50;
pub const DISBURSEMENT_STAGE_2_PERCENT: u64 = 50;
pub const MAX_URL_LEN: usize = 128;
pub const MAX_NAME_LEN: usize = 48;

#[program]
pub mod breakpoint_boost {
    use super::*;

    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        pool_name: String,
        verifier: Pubkey,
        total_stipend_per_grant: u64,
        expiry_ts: i64,
    ) -> Result<()> {
        require!(pool_name.len() <= MAX_NAME_LEN, ErrorCode::StringTooLong);
        require!(total_stipend_per_grant > 0, ErrorCode::InvalidAmount);
        require!(expiry_ts > Clock::get()?.unix_timestamp, ErrorCode::InvalidExpiry);

        let pool = &mut ctx.accounts.grant_pool;
        pool.sponsor = ctx.accounts.sponsor.key();
        pool.token_mint = ctx.accounts.token_mint.key();
        pool.vault = ctx.accounts.vault.key();
        pool.verifier = verifier;
        pool.total_stipend_per_grant = total_stipend_per_grant;
        pool.expiry_ts = expiry_ts;
        pool.total_grants_funded = 0;
        pool.active_grants_count = 0;
        pool.bump = ctx.bumps.grant_pool;
        pool.vault_bump = ctx.bumps.vault;
        pool.name = pool_name;

        emit!(PoolInitializedEvent {
            pool: pool.key(),
            sponsor: pool.sponsor,
            total_stipend: total_stipend_per_grant,
        });

        Ok(())
    }

    pub fn deposit_pool_funds(ctx: Context<DepositPoolFunds>, amount: u64) -> Result<()> {
        require!(amount > 0, ErrorCode::InvalidAmount);

        let cpi_accounts = Transfer {
            from: ctx.accounts.sponsor_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.sponsor.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        emit!(FundsDepositedEvent {
            pool: ctx.accounts.grant_pool.key(),
            amount,
        });

        Ok(())
    }

    pub fn apply_for_grant(
        ctx: Context<ApplyForGrant>,
        content_track: u8,
        content_proposal_uri: String,
        content_hash: [u8; 32],
    ) -> Result<()> {
        require!(content_proposal_uri.len() <= MAX_URL_LEN, ErrorCode::StringTooLong);
        let clock = Clock::get()?;
        require!(clock.unix_timestamp < ctx.accounts.grant_pool.expiry_ts, ErrorCode::PoolExpired);

        let application = &mut ctx.accounts.grant_application;
        application.applicant = ctx.accounts.applicant.key();
        application.pool = ctx.accounts.grant_pool.key();
        application.content_track = content_track;
        application.content_proposal_uri = content_proposal_uri;
        application.content_hash = content_hash;
        application.status = GrantStatus::Submitted as u8;
        application.milestone1_paid = false;
        application.milestone2_paid = false;
        application.submitted_at = clock.unix_timestamp;
        application.bump = ctx.bumps.grant_application;

        let pool = &mut ctx.accounts.grant_pool;
        pool.active_grants_count = pool.active_grants_count.checked_add(1).unwrap();

        emit!(ApplicationSubmittedEvent {
            pool: pool.key(),
            applicant: application.applicant,
            content_track,
        });

        Ok(())
    }

    pub fn approve_and_disburse_content(
        ctx: Context<ApproveMilestoneContent>,
        verified_hash: [u8; 32],
    ) -> Result<()> {
        let application = &mut ctx.accounts.grant_application;
        let pool = &ctx.accounts.grant_pool;

        require!(
            application.status == (GrantStatus::Submitted as u8),
            ErrorCode::InvalidStateTransition
        );
        require!(
            application.content_hash == verified_hash,
            ErrorCode::ContentHashMismatch
        );

        let payout_amount = pool
            .total_stipend_per_grant
            .checked_mul(DISBURSEMENT_STAGE_1_PERCENT)
            .unwrap()
            .checked_div(100)
            .unwrap();

        require!(
            ctx.accounts.vault.amount >= payout_amount,
            ErrorCode::InsufficientVaultBalance
        );

        let pool_key = pool.key();
        let seeds = &[
            b"vault".as_ref(),
            pool_key.as_ref(),
            &[pool.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.applicant_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, payout_amount)?;

        application.milestone1_paid = true;
        application.status = GrantStatus::ContentVerified as u8;

        emit!(MilestoneDisbursedEvent {
            pool: pool.key(),
            applicant: application.applicant,
            stage: 1,
            amount: payout_amount,
        });

        Ok(())
    }

    pub fn verify_attendance_and_finalize(
        ctx: Context<VerifyAttendanceAndFinalize>,
        ticket_verification_sig: [u8; 64],
    ) -> Result<()> {
        let application = &mut ctx.accounts.grant_application;
        let pool = &mut ctx.accounts.grant_pool;

        require!(
            application.status == (GrantStatus::ContentVerified as u8),
            ErrorCode::InvalidStateTransition
        );
        require!(application.milestone1_paid, ErrorCode::Milestone1Unpaid);
        require!(!application.milestone2_paid, ErrorCode::AlreadyClaimed);
        require!(ticket_verification_sig != [0u8; 64], ErrorCode::InvalidTicketProof);

        let payout_amount = pool
            .total_stipend_per_grant
            .checked_mul(DISBURSEMENT_STAGE_2_PERCENT)
            .unwrap()
            .checked_div(100)
            .unwrap();

        require!(
            ctx.accounts.vault.amount >= payout_amount,
            ErrorCode::InsufficientVaultBalance
        );

        let pool_key = pool.key();
        let seeds = &[
            b"vault".as_ref(),
            pool_key.as_ref(),
            &[pool.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.applicant_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, payout_amount)?;

        application.milestone2_paid = true;
        application.status = GrantStatus::Completed as u8;
        pool.total_grants_funded = pool.total_grants_funded.checked_add(1).unwrap();

        emit!(MilestoneDisbursedEvent {
            pool: pool.key(),
            applicant: application.applicant,
            stage: 2,
            amount: payout_amount,
        });

        Ok(())
    }

    pub fn emergency_reclaim_funds(ctx: Context<EmergencyReclaim>) -> Result<()> {
        let pool = &ctx.accounts.grant_pool;
        let clock = Clock::get()?;
        require!(clock.unix_timestamp > pool.expiry_ts, ErrorCode::PoolNotExpired);

        let remaining_amount = ctx.accounts.vault.amount;
        require!(remaining_amount > 0, ErrorCode::InsufficientVaultBalance);

        let pool_key = pool.key();
        let seeds = &[
            b"vault".as_ref(),
            pool_key.as_ref(),
            &[pool.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.sponsor_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, remaining_amount)?;

        emit!(PoolReclaimedEvent {
            pool: pool.key(),
            reclaimed_amount: remaining_amount,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION STRUCTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(pool_name: String)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = sponsor,
        space = 8 + GrantPool::LEN,
        seeds = [b"grant_pool", sponsor.key().as_ref(), pool_name.as_bytes()],
        bump
    )]
    pub grant_pool: Account<'info, GrantPool>,

    #[account(
        init,
        payer = sponsor,
        seeds = [b"vault", grant_pool.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = vault,
    )]
    pub vault: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositPoolFunds<'info> {
    #[account(mut)]
    pub sponsor: Signer<'info>,

    #[account(
        mut,
        has_one = sponsor @ ErrorCode::UnauthorizedSponsor,
        has_one = vault @ ErrorCode::InvalidVaultAccount,
    )]
    pub grant_pool: Account<'info, GrantPool>,

    #[account(
        mut,
        constraint = sponsor_token_account.mint == grant_pool.token_mint @ ErrorCode::MintMismatch,
        constraint = sponsor_token_account.owner == sponsor.key() @ ErrorCode::UnauthorizedSponsor
    )]
    pub sponsor_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ApplyForGrant<'info> {
    #[account(mut)]
    pub applicant: Signer<'info>,

    #[account(mut)]
    pub grant_pool: Account<'info, GrantPool>,

    #[account(
        init,
        payer = applicant,
        space = 8 + GrantApplication::LEN,
        seeds = [b"application", grant_pool.key().as_ref(), applicant.key().as_ref()],
        bump
    )]
    pub grant_application: Account<'info, GrantApplication>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ApproveMilestoneContent<'info> {
    #[account(
        mut,
        constraint = verifier.key() == grant_pool.verifier @ ErrorCode::UnauthorizedVerifier
    )]
    pub verifier: Signer<'info>,

    #[account(
        mut,
        has_one = vault @ ErrorCode::InvalidVaultAccount
    )]
    pub grant_pool: Account<'info, GrantPool>,

    #[account(
        mut,
        seeds = [b"application", grant_pool.key().as_ref(), application.applicant.as_ref()],
        bump = application.bump
    )]
    pub application: Account<'info, GrantApplication>,

    #[account(
        mut,
        constraint = applicant_token_account.owner == application.applicant @ ErrorCode::InvalidApplicantRecipient,
        constraint = applicant_token_account.mint == grant_pool.token_mint @ ErrorCode::MintMismatch
    )]
    pub applicant_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct VerifyAttendanceAndFinalize<'info> {
    #[account(
        mut,
        constraint = verifier.key() == grant_pool.verifier @ ErrorCode::UnauthorizedVerifier
    )]
    pub verifier: Signer<'info>,

    #[account(
        mut,
        has_one = vault @ ErrorCode::InvalidVaultAccount
    )]
    pub grant_pool: Account<'info, GrantPool>,

    #[account(
        mut,
        seeds = [b"application", grant_pool.key().as_ref(), application.applicant.as_ref()],
        bump = application.bump
    )]
    pub application: Account<'info, GrantApplication>,

    #[account(
        mut,
        constraint = applicant_token_account.owner == application.applicant @ ErrorCode::InvalidApplicantRecipient,
        constraint = applicant_token_account.mint == grant_pool.token_mint @ ErrorCode::MintMismatch
    )]
    pub applicant_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct EmergencyReclaim<'info> {
    #[account(
        mut,
        constraint = sponsor.key() == grant_pool.sponsor @ ErrorCode::UnauthorizedSponsor
    )]
    pub sponsor: Signer<'info>,

    #[account(
        mut,
        has_one = sponsor @ ErrorCode::UnauthorizedSponsor,
        has_one = vault @ ErrorCode::InvalidVaultAccount
    )]
    pub grant_pool: Account<'info, GrantPool>,

    #[account(
        mut,
        constraint = sponsor_token_account.owner == sponsor.key() @ ErrorCode::UnauthorizedSponsor,
        constraint = sponsor_token_account.mint == grant_pool.token_mint @ ErrorCode::MintMismatch
    )]
    pub sponsor_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// -----------------------------------------------------------------------------
// STATE DEFINITIONS
// -----------------------------------------------------------------------------

#[account]
pub struct GrantPool {
    pub sponsor: Pubkey,
    pub token_mint: Pubkey,
    pub vault: Pubkey,
    pub verifier: Pubkey,
    pub total_stipend_per_grant: u64,
    pub expiry_ts: i64,
    pub total_grants_funded: u32,
    pub active_grants_count: u32,
    pub bump: u8,
    pub vault_bump: u8,
    pub name: String,
}

impl GrantPool {
    pub const LEN: usize = 32 + 32 + 32 + 32 + 8 + 8 + 4 + 4 + 1 + 1 + (4 + MAX_NAME_LEN);
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum GrantStatus {
    Submitted = 0,
    ContentVerified = 1,
    Completed = 2,
    Revoked = 3,
}

#[account]
pub struct GrantApplication {
    pub applicant: Pubkey,
    pub pool: Pubkey,
    pub content_track: u8,
    pub content_proposal_uri: String,
    pub content_hash: [u8; 32],
    pub status: u8,
    pub milestone1_paid: bool,
    pub milestone2_paid: bool,
    pub submitted_at: i64,
    pub bump: u8,
}

impl GrantApplication {
    pub const LEN: usize = 32 + 32 + 1 + (4 + MAX_URL_LEN) + 32 + 1 + 1 + 1 + 8 + 1;
}

// -----------------------------------------------------------------------------
// EVENTS
// -----------------------------------------------------------------------------

#[event]
pub struct PoolInitializedEvent {
    pub pool: Pubkey,
    pub sponsor: Pubkey,
    pub total_stipend: u64,
}

#[event]
pub struct FundsDepositedEvent {
    pub pool: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ApplicationSubmittedEvent {
    pub pool: Pubkey,
    pub applicant: Pubkey,
    pub content_track: u8,
}

#[event]
pub struct MilestoneDisbursedEvent {
    pub pool: Pubkey,
    pub applicant: Pubkey,
    pub stage: u8,
    pub amount: u64,
}

#[event]
pub struct PoolReclaimedEvent {
    pub pool: Pubkey,
    pub reclaimed_amount: u64,
}

// -----------------------------------------------------------------------------
// ERROR CODES
// -----------------------------------------------------------------------------

#[error_code]
pub enum ErrorCode {
    #[msg("String exceeds maximum allowable length")]
    StringTooLong,
    #[msg("Specified numerical amount must be greater than zero")]
    InvalidAmount,
    #[msg("Pool expiry timestamp must be set in the future")]
    InvalidExpiry,
    #[msg("Sponsor is not authorized for this action")]
    UnauthorizedSponsor,
    #[msg("Verifier is not authorized for milestone validation")]
    UnauthorizedVerifier,
    #[msg("Provided token vault does not match the registered pool account")]
    InvalidVaultAccount,
    #[msg("Token account mint mismatch")]
    MintMismatch,
    #[msg("Grant pool has already reached its expiry timestamp")]
    PoolExpired,
    #[msg("Grant pool expiry timestamp has not been reached yet")]
    PoolNotExpired,
    #[msg("Application is not in a valid state for this milestone transition")]
    InvalidStateTransition,
    #[msg("Calculated SHA256 content hash does not match submission signature")]
    ContentHashMismatch,
    #[msg("Vault does not have sufficient balance to pay stipend")]
    InsufficientVaultBalance,
    #[msg("Recipient token account does not match applicant owner key")]
    InvalidApplicantRecipient,
    #[msg("Milestone 1 travel boost has not been disbursed")]
    Milestone1Unpaid,
    #[msg("Final milestone has already been claimed")]
    AlreadyClaimed,
    #[msg("Invalid Breakpoint 2026 ticket verification signature")]
    InvalidTicketProof,
}