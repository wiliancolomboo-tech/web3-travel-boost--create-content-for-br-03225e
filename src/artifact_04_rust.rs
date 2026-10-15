use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("BrkBt6WzB9gTq8X3gZg6jR1Z9qjXGzXGzXGzXGzXGzXX");

pub const MAX_TITLE_LEN: usize = 64;
pub const MAX_URI_LEN: usize = 128;
pub const MAX_MEMBERS: usize = 5;

#[program]
pub mod breakboost {
    use super::*;

    /// Initializes a community travel campaign for Solana Breakpoint 2026.
    pub fn initialize_campaign(
        ctx: Context<InitializeCampaign>,
        campaign_name: String,
        total_budget: u64,
        max_stipend_per_creator: u64,
    ) -> Result<()> {
        require!(
            campaign_name.len() <= MAX_TITLE_LEN,
            BreakBoostError::StringTooLong
        );
        require!(total_budget > 0, BreakBoostError::InvalidAmount);
        require!(
            max_stipend_per_creator > 0 && max_stipend_per_creator <= total_budget,
            BreakBoostError::InvalidStipendLimit
        );

        let campaign = &mut ctx.accounts.campaign;
        campaign.authority = ctx.accounts.authority.key();
        campaign.funding_mint = ctx.accounts.funding_mint.key();
        campaign.vault = ctx.accounts.vault.key();
        campaign.total_budget = total_budget;
        campaign.allocated_budget = 0;
        campaign.disbursed_budget = 0;
        campaign.max_stipend = max_stipend_per_creator;
        campaign.proposal_count = 0;
        campaign.bump = ctx.bumps.campaign;
        campaign.vault_bump = ctx.bumps.vault;

        emit!(CampaignInitializedEvent {
            campaign: campaign.key(),
            authority: campaign.authority,
            total_budget,
            max_stipend: max_stipend_per_creator,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Deposits rewards into the Breakpoint Travel Vault.
    pub fn fund_campaign(ctx: Context<FundCampaign>, amount: u64) -> Result<()> {
        require!(amount > 0, BreakBoostError::InvalidAmount);

        let cpi_accounts = Transfer {
            from: ctx.accounts.funder_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.funder.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let campaign = &mut ctx.accounts.campaign;
        campaign.total_budget = campaign
            .total_budget
            .checked_add(amount)
            .ok_or(BreakBoostError::MathOverflow)?;

        emit!(CampaignFundedEvent {
            campaign: campaign.key(),
            funder: ctx.accounts.funder.key(),
            amount,
            new_total: campaign.total_budget,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Content creators apply for a Breakpoint 2026 Travel Boost stipend.
    pub fn create_proposal(
        ctx: Context<CreateProposal>,
        title: String,
        content_uri: String,
        content_hash: [u8; 32],
        requested_amount: u64,
    ) -> Result<()> {
        require!(title.len() <= MAX_TITLE_LEN, BreakBoostError::StringTooLong);
        require!(content_uri.len() <= MAX_URI_LEN, BreakBoostError::StringTooLong);
        require!(requested_amount > 0, BreakBoostError::InvalidAmount);

        let campaign = &mut ctx.accounts.campaign;
        require!(
            requested_amount <= campaign.max_stipend,
            BreakBoostError::ExceedsMaxStipend
        );

        let available_budget = campaign
            .total_budget
            .checked_sub(campaign.allocated_budget)
            .ok_or(BreakBoostError::InsufficientCampaignBudget)?;
        require!(
            requested_amount <= available_budget,
            BreakBoostError::InsufficientCampaignBudget
        );

        let proposal_index = campaign.proposal_count;
        campaign.proposal_count = campaign
            .proposal_count
            .checked_add(1)
            .ok_or(BreakBoostError::MathOverflow)?;

        campaign.allocated_budget = campaign
            .allocated_budget
            .checked_add(requested_amount)
            .ok_or(BreakBoostError::MathOverflow)?;

        let proposal = &mut ctx.accounts.proposal;
        proposal.campaign = campaign.key();
        proposal.creator = ctx.accounts.creator.key();
        proposal.proposal_index = proposal_index;
        proposal.title = title;
        proposal.content_uri = content_uri;
        proposal.content_hash = content_hash;
        proposal.requested_amount = requested_amount;
        proposal.status = ProposalStatus::Pending;
        proposal.created_at = Clock::get()?.unix_timestamp;
        proposal.verified_at = 0;
        proposal.bump = ctx.bumps.proposal;

        emit!(ProposalCreatedEvent {
            campaign: campaign.key(),
            proposal: proposal.key(),
            creator: proposal.creator,
            requested_amount,
            content_hash,
            timestamp: proposal.created_at,
        });

        Ok(())
    }

    /// Campaign curators review and approve or reject travel boost proposals.
    pub fn review_proposal(
        ctx: Context<ReviewProposal>,
        approve: bool,
    ) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let campaign = &mut ctx.accounts.campaign;

        require!(
            proposal.status == ProposalStatus::Pending,
            BreakBoostError::InvalidProposalStatus
        );

        if approve {
            proposal.status = ProposalStatus::Approved;
            emit!(ProposalReviewedEvent {
                proposal: proposal.key(),
                status: ProposalStatus::Approved,
                reviewer: ctx.accounts.authority.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        } else {
            proposal.status = ProposalStatus::Rejected;
            // Deallocate the reserved budget back to the campaign
            campaign.allocated_budget = campaign
                .allocated_budget
                .checked_sub(proposal.requested_amount)
                .ok_or(BreakBoostError::MathOverflow)?;

            emit!(ProposalReviewedEvent {
                proposal: proposal.key(),
                status: ProposalStatus::Rejected,
                reviewer: ctx.accounts.authority.key(),
                timestamp: Clock::get()?.unix_timestamp,
            });
        }

        Ok(())
    }

    /// Verifies live content proof and releases travel stipend funds directly to creator.
    pub fn verify_and_disburse_stipend(
        ctx: Context<DisburseStipend>,
        final_proof_hash: [u8; 32],
    ) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let campaign = &mut ctx.accounts.campaign;

        require!(
            proposal.status == ProposalStatus::Approved,
            BreakBoostError::InvalidProposalStatus
        );
        require!(
            final_proof_hash == proposal.content_hash,
            BreakBoostError::ProofHashMismatch
        );

        let amount_to_pay = proposal.requested_amount;

        // Transfer funds from PDA vault to creator token account
        let campaign_key = campaign.key();
        let seeds = &[
            b"vault".as_ref(),
            campaign_key.as_ref(),
            &[campaign.vault_bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.creator_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );
        token::transfer(cpi_ctx, amount_to_pay)?;

        // Update states
        campaign.disbursed_budget = campaign
            .disbursed_budget
            .checked_add(amount_to_pay)
            .ok_or(BreakBoostError::MathOverflow)?;

        proposal.status = ProposalStatus::Disbursed;
        proposal.verified_at = Clock::get()?.unix_timestamp;

        emit!(StipendDisbursedEvent {
            campaign: campaign.key(),
            proposal: proposal.key(),
            creator: proposal.creator,
            amount: amount_to_pay,
            timestamp: proposal.verified_at,
        });

        Ok(())
    }
}

// ==========================================
// ACCOUNT VALIDATION STRUCTS
// ==========================================

#[derive(Accounts)]
#[instruction(campaign_name: String)]
pub struct InitializeCampaign<'info> {
    #[account(
        init,
        payer = authority,
        space = Campaign::LEN,
        seeds = [b"campaign", authority.key().as_ref(), campaign_name.as_bytes()],
        bump
    )]
    pub campaign: Account<'info, Campaign>,

    pub funding_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        seeds = [b"vault", campaign.key().as_ref()],
        bump,
        token::mint = funding_mint,
        token::authority = vault
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct FundCampaign<'info> {
    #[account(
        mut,
        has_one = vault @ BreakBoostError::InvalidVaultAccount
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = funder_token_account.mint == campaign.funding_mint @ BreakBoostError::InvalidMint
    )]
    pub funder_token_account: Account<'info, TokenAccount>,

    pub funder: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = creator,
        space = Proposal::LEN,
        seeds = [
            b"proposal",
            campaign.key().as_ref(),
            &campaign.proposal_count.to_le_bytes()
        ],
        bump
    )]
    pub proposal: Account<'info, Proposal>,

    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReviewProposal<'info> {
    #[account(
        mut,
        has_one = authority @ BreakBoostError::Unauthorized
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        has_one = campaign @ BreakBoostError::ProposalCampaignMismatch
    )]
    pub proposal: Account<'info, Proposal>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct DisburseStipend<'info> {
    #[account(
        mut,
        has_one = authority @ BreakBoostError::Unauthorized,
        has_one = vault @ BreakBoostError::InvalidVaultAccount
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        has_one = campaign @ BreakBoostError::ProposalCampaignMismatch
    )]
    pub proposal: Account<'info, Proposal>,

    #[account(
        mut,
        seeds = [b"vault", campaign.key().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = creator_token_account.owner == proposal.creator @ BreakBoostError::InvalidRecipientTokenAccount,
        constraint = creator_token_account.mint == campaign.funding_mint @ BreakBoostError::InvalidMint
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// ==========================================
// DATA STRUCTURES & ENUMS
// ==========================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Disbursed,
}

#[account]
pub struct Campaign {
    pub authority: Pubkey,         // 32
    pub funding_mint: Pubkey,      // 32
    pub vault: Pubkey,             // 32
    pub total_budget: u64,         // 8
    pub allocated_budget: u64,     // 8
    pub disbursed_budget: u64,     // 8
    pub max_stipend: u64,          // 8
    pub proposal_count: u64,       // 8
    pub bump: u8,                  // 1
    pub vault_bump: u8,            // 1
}

impl Campaign {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 64; // with padding
}

#[account]
pub struct Proposal {
    pub campaign: Pubkey,          // 32
    pub creator: Pubkey,           // 32
    pub proposal_index: u64,       // 8
    pub requested_amount: u64,     // 8
    pub content_hash: [u8; 32],    // 32
    pub status: ProposalStatus,    // 1 + 1
    pub created_at: i64,           // 8
    pub verified_at: i64,          // 8
    pub title: String,             // 4 + 64
    pub content_uri: String,       // 4 + 128
    pub bump: u8,                  // 1
}

impl Proposal {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 8 + 32 + 2 + 8 + 8 + (4 + 64) + (4 + 128) + 1 + 32;
}

// ==========================================
// EVENTS & ERRORS
// ==========================================

#[event]
pub struct CampaignInitializedEvent {
    pub campaign: Pubkey,
    pub authority: Pubkey,
    pub total_budget: u64,
    pub max_stipend: u64,
    pub timestamp: i64,
}

#[event]
pub struct CampaignFundedEvent {
    pub campaign: Pubkey,
    pub funder: Pubkey,
    pub amount: u64,
    pub new_total: u64,
    pub timestamp: i64,
}

#[event]
pub struct ProposalCreatedEvent {
    pub campaign: Pubkey,
    pub proposal: Pubkey,
    pub creator: Pubkey,
    pub requested_amount: u64,
    pub content_hash: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct ProposalReviewedEvent {
    pub proposal: Pubkey,
    pub status: ProposalStatus,
    pub reviewer: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct StipendDisbursedEvent {
    pub campaign: Pubkey,
    pub proposal: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum BreakBoostError {
    #[msg("String exceeds maximum allowable characters")]
    StringTooLong,
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Max stipend limit must be greater than 0 and less than total budget")]
    InvalidStipendLimit,
    #[msg("Requested stipend exceeds maximum allowance per creator")]
    ExceedsMaxStipend,
    #[msg("Insufficient campaign budget remaining for this proposal")]
    InsufficientCampaignBudget,
    #[msg("Proposal status is invalid for this operation")]
    InvalidProposalStatus,
    #[msg("Content SHA-256 proof does not match committed hash")]
    ProofHashMismatch,
    #[msg("Signer is not authorized to execute this action")]
    Unauthorized,
    #[msg("Associated mint does not match campaign currency")]
    InvalidMint,
    #[msg("Vault address does not match expected PDA")]
    InvalidVaultAccount,
    #[msg("Recipient token account does not match proposal author")]
    InvalidRecipientTokenAccount,
    #[msg("Proposal does not belong to specified campaign")]
    ProposalCampaignMismatch,
    #[msg("Arithmetic overflow detected")]
    MathOverflow,
}