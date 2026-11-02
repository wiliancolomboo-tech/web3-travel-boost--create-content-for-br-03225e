use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("BpBstJcZ1Ea823GfLhT6Nq4Zg1kC3v5H8w9s8v7n6m5L");

pub const DISCRIMINATOR_LENGTH: usize = 8;
pub const MAX_CID_LENGTH: usize = 64;
pub const MAX_TITLE_LENGTH: usize = 128;
pub const MAX_REVIEWS_ALLOWED: usize = 5;

#[program]
pub mod breakpoint_boost {
    use super::*;

    /// Initialize a new Breakpoint Travel Grant Campaign.
    pub fn initialize_campaign(
        ctx: Context<InitializeCampaign>,
        campaign_id: u64,
        grant_per_recipient: u64,
        max_recipients: u32,
        required_approvals: u8,
        deadline: i64,
    ) -> Result<()> {
        require!(
            required_approvals > 0 && required_approvals as usize <= MAX_REVIEWS_ALLOWED,
            BoostError::InvalidApprovalThreshold
        );
        require!(max_recipients > 0, BoostError::InvalidMaxRecipients);
        require!(grant_per_recipient > 0, BoostError::InvalidGrantAmount);

        let clock = Clock::get()?;
        require!(deadline > clock.unix_timestamp, BoostError::InvalidDeadline);

        let campaign = &mut ctx.accounts.campaign;
        campaign.admin = ctx.accounts.admin.key();
        campaign.mint = ctx.accounts.mint.key();
        campaign.campaign_id = campaign_id;
        campaign.grant_per_recipient = grant_per_recipient;
        campaign.max_recipients = max_recipients;
        campaign.current_recipients = 0;
        campaign.required_approvals = required_approvals;
        campaign.deadline = deadline;
        campaign.vault_bump = ctx.bumps.vault;
        campaign.bump = ctx.bumps.campaign;

        emit!(CampaignCreatedEvent {
            campaign_id,
            admin: ctx.accounts.admin.key(),
            mint: ctx.accounts.mint.key(),
            grant_per_recipient,
            max_recipients,
            deadline,
        });

        Ok(())
    }

    /// Deposit funds into the Campaign Escrow Vault.
    pub fn fund_campaign(ctx: Context<FundCampaign>, amount: u64) -> Result<()> {
        require!(amount > 0, BoostError::InvalidFundingAmount);

        let cpi_accounts = Transfer {
            from: ctx.accounts.funder_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.funder.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        emit!(CampaignFundedEvent {
            campaign_id: ctx.accounts.campaign.campaign_id,
            funder: ctx.accounts.funder.key(),
            amount,
        });

        Ok(())
    }

    /// Content Creator submits technical content for Breakpoint 2026.
    pub fn submit_content(
        ctx: Context<SubmitContent>,
        content_id: u64,
        cid: String,
        title: String,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let campaign = &ctx.accounts.campaign;

        require!(clock.unix_timestamp <= campaign.deadline, BoostError::CampaignExpired);
        require!(
            campaign.current_recipients < campaign.max_recipients,
            BoostError::CampaignCapReached
        );
        require!(
            !cid.is_empty() && cid.len() <= MAX_CID_LENGTH,
            BoostError::InvalidCidLength
        );
        require!(
            !title.is_empty() && title.len() <= MAX_TITLE_LENGTH,
            BoostError::InvalidTitleLength
        );

        let submission = &mut ctx.accounts.submission;
        submission.creator = ctx.accounts.creator.key();
        submission.campaign = campaign.key();
        submission.content_id = content_id;
        submission.cid = cid.clone();
        submission.title = title.clone();
        submission.approvals_count = 0;
        submission.disbursed = false;
        submission.created_at = clock.unix_timestamp;
        submission.bump = ctx.bumps.submission;

        emit!(ContentSubmittedEvent {
            campaign_id: campaign.campaign_id,
            content_id,
            creator: ctx.accounts.creator.key(),
            cid,
            title,
        });

        Ok(())
    }

    /// Reviewer or automated oracle validator approves submitted content.
    pub fn review_content(
        ctx: Context<ReviewContent>,
        evaluation_score: u8,
    ) -> Result<()> {
        require!(evaluation_score >= 70, BoostError::ScoreBelowThreshold);
        let submission = &mut ctx.accounts.submission;
        require!(!submission.disbursed, BoostError::AlreadyDisbursed);

        for i in 0..submission.approvals_count as usize {
            require!(
                submission.reviewers[i] != ctx.accounts.reviewer.key(),
                BoostError::ReviewerAlreadyAttested
            );
        }

        let idx = submission.approvals_count as usize;
        require!(idx < MAX_REVIEWS_ALLOWED, BoostError::MaxReviewsExceeded);

        submission.reviewers[idx] = ctx.accounts.reviewer.key();
        submission.approvals_count = submission.approvals_count.checked_add(1).ok_or(BoostError::MathOverflow)?;

        emit!(ContentReviewedEvent {
            campaign_id: ctx.accounts.campaign.campaign_id,
            content_id: submission.content_id,
            reviewer: ctx.accounts.reviewer.key(),
            score: evaluation_score,
            total_approvals: submission.approvals_count,
        });

        Ok(())
    }

    /// Disburse travel grant upon reaching the approval threshold.
    pub fn disburse_grant(ctx: Context<DisburseGrant>) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        let submission = &mut ctx.accounts.submission;

        require!(!submission.disbursed, BoostError::AlreadyDisbursed);
        require!(
            submission.approvals_count >= campaign.required_approvals,
            BoostError::InsufficientApprovals
        );
        require!(
            campaign.current_recipients < campaign.max_recipients,
            BoostError::CampaignCapReached
        );

        let grant_amount = campaign.grant_per_recipient;
        let campaign_id_bytes = campaign.campaign_id.to_le_bytes();
        let seeds = &[
            b"vault".as_ref(),
            campaign_id_bytes.as_ref(),
            &[campaign.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        token::transfer(cpi_ctx, grant_amount)?;

        submission.disbursed = true;
        campaign.current_recipients = campaign.current_recipients.checked_add(1).ok_or(BoostError::MathOverflow)?;

        emit!(GrantDisbursedEvent {
            campaign_id: campaign.campaign_id,
            content_id: submission.content_id,
            recipient: submission.creator,
            amount: grant_amount,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION STRUCTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(campaign_id: u64)]
pub struct InitializeCampaign<'info> {
    #[account(
        init,
        payer = admin,
        space = DISCRIMINATOR_LENGTH + Campaign::LEN,
        seeds = [b"campaign", campaign_id.to_le_bytes().as_ref()],
        bump
    )]
    pub campaign: Account<'info, Campaign>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        seeds = [b"vault", campaign_id.to_le_bytes().as_ref()],
        bump,
        token::mint = mint,
        token::authority = vault
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct FundCampaign<'info> {
    #[account(
        mut,
        seeds = [b"campaign", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [b"vault", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = funder_token_account.mint == campaign.mint @ BoostError::InvalidMint,
        constraint = funder_token_account.owner == funder.key() @ BoostError::Unauthorized
    )]
    pub funder_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub funder: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(content_id: u64)]
pub struct SubmitContent<'info> {
    #[account(
        mut,
        seeds = [b"campaign", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = creator,
        space = DISCRIMINATOR_LENGTH + Submission::LEN,
        seeds = [
            b"submission",
            campaign.campaign_id.to_le_bytes().as_ref(),
            content_id.to_le_bytes().as_ref(),
            creator.key().as_ref()
        ],
        bump
    )]
    pub submission: Account<'info, Submission>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReviewContent<'info> {
    #[account(
        seeds = [b"campaign", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [
            b"submission",
            campaign.campaign_id.to_le_bytes().as_ref(),
            submission.content_id.to_le_bytes().as_ref(),
            submission.creator.as_ref()
        ],
        bump = submission.bump
    )]
    pub submission: Account<'info, Submission>,

    pub reviewer: Signer<'info>,
}

#[derive(Accounts)]
pub struct DisburseGrant<'info> {
    #[account(
        mut,
        seeds = [b"campaign", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [
            b"submission",
            campaign.campaign_id.to_le_bytes().as_ref(),
            submission.content_id.to_le_bytes().as_ref(),
            submission.creator.as_ref()
        ],
        bump = submission.bump
    )]
    pub submission: Account<'info, Submission>,

    #[account(
        mut,
        seeds = [b"vault", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = recipient_token_account.mint == campaign.mint @ BoostError::InvalidMint,
        constraint = recipient_token_account.owner == submission.creator @ BoostError::InvalidRecipientTokenAccount
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// -----------------------------------------------------------------------------
// STATE ACCOUNTS
// -----------------------------------------------------------------------------

#[account]
pub struct Campaign {
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub campaign_id: u64,
    pub grant_per_recipient: u64,
    pub max_recipients: u32,
    pub current_recipients: u32,
    pub required_approvals: u8,
    pub deadline: i64,
    pub vault_bump: u8,
    pub bump: u8,
}

impl Campaign {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 4 + 4 + 1 + 8 + 1 + 1;
}

#[account]
pub struct Submission {
    pub creator: Pubkey,
    pub campaign: Pubkey,
    pub content_id: u64,
    pub cid: String,
    pub title: String,
    pub reviewers: [Pubkey; MAX_REVIEWS_ALLOWED],
    pub approvals_count: u8,
    pub disbursed: bool,
    pub created_at: i64,
    pub bump: u8,
}

impl Submission {
    pub const LEN: usize = 32
        + 32
        + 8
        + (4 + MAX_CID_LENGTH)
        + (4 + MAX_TITLE_LENGTH)
        + (32 * MAX_REVIEWS_ALLOWED)
        + 1
        + 1
        + 8
        + 1;
}

// -----------------------------------------------------------------------------
// EVENTS & ERRORS
// -----------------------------------------------------------------------------

#[event]
pub struct CampaignCreatedEvent {
    pub campaign_id: u64,
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub grant_per_recipient: u64,
    pub max_recipients: u32,
    pub deadline: i64,
}

#[event]
pub struct CampaignFundedEvent {
    pub campaign_id: u64,
    pub funder: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ContentSubmittedEvent {
    pub campaign_id: u64,
    pub content_id: u64,
    pub creator: Pubkey,
    pub cid: String,
    pub title: String,
}

#[event]
pub struct ContentReviewedEvent {
    pub campaign_id: u64,
    pub content_id: u64,
    pub reviewer: Pubkey,
    pub score: u8,
    pub total_approvals: u8,
}

#[event]
pub struct GrantDisbursedEvent {
    pub campaign_id: u64,
    pub content_id: u64,
    pub recipient: Pubkey,
    pub amount: u64,
}

#[error_code]
pub enum BoostError {
    #[msg("Invalid approval threshold.")]
    InvalidApprovalThreshold,
    #[msg("Invalid max recipients.")]
    InvalidMaxRecipients,
    #[msg("Invalid grant amount.")]
    InvalidGrantAmount,
    #[msg("Invalid deadline.")]
    InvalidDeadline,
    #[msg("Funding amount must be greater than zero.")]
    InvalidFundingAmount,
    #[msg("Campaign deadline has passed.")]
    CampaignExpired,
    #[msg("Maximum grant recipients already reached.")]
    CampaignCapReached,
    #[msg("CID string exceeds allowed length.")]
    InvalidCidLength,
    #[msg("Title string exceeds allowed length.")]
    InvalidTitleLength,
    #[msg("Content evaluation score is below threshold.")]
    ScoreBelowThreshold,
    #[msg("Grant has already been disbursed.")]
    AlreadyDisbursed,
    #[msg("Reviewer has already approved this submission.")]
    ReviewerAlreadyAttested,
    #[msg("Maximum reviews reached for submission.")]
    MaxReviewsExceeded,
    #[msg("Not enough reviewer approvals to disburse grant.")]
    InsufficientApprovals,
    #[msg("Mismatched SPL token mint.")]
    InvalidMint,
    #[msg("Recipient token account owner does not match creator.")]
    InvalidRecipientTokenAccount,
    #[msg("Arithmetic overflow.")]
    MathOverflow,
    #[msg("Unauthorized invocation.")]
    Unauthorized,
}