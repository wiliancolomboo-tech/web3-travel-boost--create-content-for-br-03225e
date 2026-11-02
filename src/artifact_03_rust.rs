use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("TrvL8o2sTxE9mYpB6uA12KzQ4bFv4jXm9KzT7mP1vN8");

#[program]
pub mod travel_boost {
    use super::*;

    pub fn initialize_campaign(
        ctx: Context<InitializeCampaign>,
        title: String,
        description: String,
        deadline_ts: i64,
        max_recipients: u32,
        tier_allowances: [u64; 3], // [Micro: 500, Regional: 1200, Global: 2500] in base units
    ) -> Result<()> {
        require!(title.len() <= 64, TravelBoostError::StringTooLong);
        require!(description.len() <= 256, TravelBoostError::StringTooLong);
        require!(
            deadline_ts > Clock::get()?.unix_timestamp,
            TravelBoostError::InvalidDeadline
        );
        require!(max_recipients > 0, TravelBoostError::InvalidRecipientCount);

        let campaign = &mut ctx.accounts.campaign;
        campaign.authority = ctx.accounts.authority.key();
        campaign.token_mint = ctx.accounts.token_mint.key();
        campaign.vault = ctx.accounts.vault.key();
        campaign.title = title;
        campaign.description = description;
        campaign.deadline_ts = deadline_ts;
        campaign.max_recipients = max_recipients;
        campaign.current_recipients = 0;
        campaign.total_funds_escrowed = 0;
        campaign.total_funds_disbursed = 0;
        campaign.tier_allowances = tier_allowances;
        campaign.bump = ctx.bumps.campaign;
        campaign.vault_bump = ctx.bumps.vault;

        emit!(CampaignInitialized {
            campaign: campaign.key(),
            authority: campaign.authority,
            deadline_ts,
        });

        Ok(())
    }

    pub fn deposit_funds(ctx: Context<DepositFunds>, amount: u64) -> Result<()> {
        require!(amount > 0, TravelBoostError::InvalidAmount);

        let cpi_accounts = Transfer {
            from: ctx.accounts.funder_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.funder.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let campaign = &mut ctx.accounts.campaign;
        campaign.total_funds_escrowed = campaign
            .total_funds_escrowed
            .checked_add(amount)
            .ok_or(TravelBoostError::MathOverflow)?;

        emit!(FundsDeposited {
            campaign: campaign.key(),
            funder: ctx.accounts.funder.key(),
            amount,
        });

        Ok(())
    }

    pub fn register_creator(
        ctx: Context<RegisterCreator>,
        handle: String,
        requested_tier: u8,
    ) -> Result<()> {
        require!(handle.len() <= 32, TravelBoostError::StringTooLong);
        require!(requested_tier < 3, TravelBoostError::InvalidTier);

        let campaign = &ctx.accounts.campaign;
        require!(
            Clock::get()?.unix_timestamp < campaign.deadline_ts,
            TravelBoostError::CampaignExpired
        );

        let application = &mut ctx.accounts.application;
        application.campaign = campaign.key();
        application.creator = ctx.accounts.creator.key();
        application.handle = handle;
        application.requested_tier = requested_tier;
        application.status = ApplicationStatus::Submitted;
        application.content_uri = String::new();
        application.approved_grant_amount = 0;
        application.created_at = Clock::get()?.unix_timestamp;
        application.bump = ctx.bumps.application;

        emit!(CreatorRegistered {
            campaign: campaign.key(),
            creator: application.creator,
            tier: requested_tier,
        });

        Ok(())
    }

    pub fn submit_content(ctx: Context<SubmitContent>, content_uri: String) -> Result<()> {
        require!(content_uri.len() <= 200, TravelBoostError::StringTooLong);
        let application = &mut ctx.accounts.application;
        require!(
            application.status == ApplicationStatus::Submitted
                || application.status == ApplicationStatus::ChangesRequested,
            TravelBoostError::InvalidState
        );

        application.content_uri = content_uri.clone();
        application.status = ApplicationStatus::UnderReview;

        emit!(ContentSubmitted {
            campaign: application.campaign,
            creator: application.creator,
            content_uri,
        });

        Ok(())
    }

    pub fn attest_and_evaluate(
        ctx: Context<AttestAndEvaluate>,
        quality_score: u8, // Scale 1 - 100
        approved: bool,
        grant_override: Option<u64>,
    ) -> Result<()> {
        require!(quality_score <= 100, TravelBoostError::InvalidScore);

        let campaign = &mut ctx.accounts.campaign;
        let application = &mut ctx.accounts.application;

        require!(
            application.status == ApplicationStatus::UnderReview,
            TravelBoostError::InvalidState
        );

        if approved {
            require!(
                campaign.current_recipients < campaign.max_recipients,
                TravelBoostError::MaxRecipientsReached
            );

            let grant_amount = match grant_override {
                Some(amt) => amt,
                None => campaign.tier_allowances[application.requested_tier as usize],
            };

            let remaining_balance = ctx.accounts.vault.amount;
            require!(
                remaining_balance >= grant_amount,
                TravelBoostError::InsufficientVaultFunds
            );

            application.status = ApplicationStatus::Approved;
            application.approved_grant_amount = grant_amount;
            campaign.current_recipients = campaign
                .current_recipients
                .checked_add(1)
                .ok_or(TravelBoostError::MathOverflow)?;

            emit!(GrantApproved {
                campaign: campaign.key(),
                creator: application.creator,
                amount: grant_amount,
                quality_score,
            });
        } else {
            application.status = ApplicationStatus::Rejected;
            emit!(GrantRejected {
                campaign: campaign.key(),
                creator: application.creator,
                quality_score,
            });
        }

        Ok(())
    }

    pub fn claim_travel_grant(ctx: Context<ClaimTravelGrant>) -> Result<()> {
        let application = &mut ctx.accounts.application;
        require!(
            application.status == ApplicationStatus::Approved,
            TravelBoostError::InvalidState
        );
        require!(
            application.approved_grant_amount > 0,
            TravelBoostError::ZeroGrantAmount
        );

        let grant_amount = application.approved_grant_amount;
        let campaign_key = ctx.accounts.campaign.key();
        let bump = ctx.accounts.campaign.vault_bump;
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"campaign-vault",
            campaign_key.as_ref(),
            &[bump],
        ]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.creator_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, grant_amount)?;

        application.status = ApplicationStatus::Disbursed;
        let campaign = &mut ctx.accounts.campaign;
        campaign.total_funds_disbursed = campaign
            .total_funds_disbursed
            .checked_add(grant_amount)
            .ok_or(TravelBoostError::MathOverflow)?;

        emit!(GrantDisbursed {
            campaign: campaign.key(),
            creator: application.creator,
            amount: grant_amount,
        });

        Ok(())
    }

    pub fn emergency_clawback(ctx: Context<EmergencyClawback>) -> Result<()> {
        let campaign = &ctx.accounts.campaign;
        require!(
            Clock::get()?.unix_timestamp >= campaign.deadline_ts,
            TravelBoostError::CampaignNotExpired
        );

        let remaining_balance = ctx.accounts.vault.amount;
        require!(remaining_balance > 0, TravelBoostError::ZeroVaultBalance);

        let campaign_key = campaign.key();
        let bump = campaign.vault_bump;
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"campaign-vault",
            campaign_key.as_ref(),
            &[bump],
        ]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.authority_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, remaining_balance)?;

        emit!(ClawbackExecuted {
            campaign: campaign.key(),
            amount: remaining_balance,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION CONTEXTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(title: String)]
pub struct InitializeCampaign<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Campaign::INIT_SPACE,
        seeds = [b"travel-campaign", authority.key().as_ref(), title.as_bytes()],
        bump
    )]
    pub campaign: Account<'info, Campaign>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        seeds = [b"campaign-vault", campaign.key().as_ref()],
        bump,
        token::mint = token_mint,
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
pub struct DepositFunds<'info> {
    #[account(mut)]
    pub campaign: Account<'info, Campaign>,
    #[account(
        mut,
        seeds = [b"campaign-vault", campaign.key().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub funder_token_account: Account<'info, TokenAccount>,
    pub funder: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RegisterCreator<'info> {
    #[account(mut)]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = creator,
        space = 8 + CreatorApplication::INIT_SPACE,
        seeds = [b"creator-application", campaign.key().as_ref(), creator.key().as_ref()],
        bump
    )]
    pub application: Account<'info, CreatorApplication>,

    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitContent<'info> {
    pub campaign: Account<'info, Campaign>,
    #[account(
        mut,
        seeds = [b"creator-application", campaign.key().as_ref(), creator.key().as_ref()],
        bump = application.bump,
        has_one = creator
    )]
    pub application: Account<'info, CreatorApplication>,
    pub creator: Signer<'info>,
}

#[derive(Accounts)]
pub struct AttestAndEvaluate<'info> {
    #[account(
        mut,
        has_one = authority,
        has_one = vault
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [b"creator-application", campaign.key().as_ref(), application.creator.as_ref()],
        bump = application.bump
    )]
    pub application: Account<'info, CreatorApplication>,

    #[account(
        mut,
        seeds = [b"campaign-vault", campaign.key().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimTravelGrant<'info> {
    #[account(
        mut,
        has_one = vault
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [b"creator-application", campaign.key().as_ref(), creator.key().as_ref()],
        bump = application.bump,
        has_one = creator
    )]
    pub application: Account<'info, CreatorApplication>,

    #[account(
        mut,
        seeds = [b"campaign-vault", campaign.key().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = campaign.token_mint,
        token::authority = creator
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    pub creator: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct EmergencyClawback<'info> {
    #[account(
        mut,
        has_one = authority,
        has_one = vault
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [b"campaign-vault", campaign.key().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = campaign.token_mint,
        token::authority = authority
    )]
    pub authority_token_account: Account<'info, TokenAccount>,

    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// -----------------------------------------------------------------------------
// STATE STRUCTURES
// -----------------------------------------------------------------------------

#[account]
#[derive(InitSpace)]
pub struct Campaign {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub vault: Pubkey,
    #[max_len(64)]
    pub title: String,
    #[max_len(256)]
    pub description: String,
    pub deadline_ts: i64,
    pub max_recipients: u32,
    pub current_recipients: u32,
    pub total_funds_escrowed: u64,
    pub total_funds_disbursed: u64,
    pub tier_allowances: [u64; 3],
    pub bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct CreatorApplication {
    pub campaign: Pubkey,
    pub creator: Pubkey,
    #[max_len(32)]
    pub handle: String,
    pub requested_tier: u8,
    pub status: ApplicationStatus,
    #[max_len(200)]
    pub content_uri: String,
    pub approved_grant_amount: u64,
    pub created_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum ApplicationStatus {
    Submitted,
    UnderReview,
    ChangesRequested,
    Approved,
    Rejected,
    Disbursed,
}

// -----------------------------------------------------------------------------
// EVENTS & ERRORS
// -----------------------------------------------------------------------------

#[event]
pub struct CampaignInitialized {
    pub campaign: Pubkey,
    pub authority: Pubkey,
    pub deadline_ts: i64,
}

#[event]
pub struct FundsDeposited {
    pub campaign: Pubkey,
    pub funder: Pubkey,
    pub amount: u64,
}

#[event]
pub struct CreatorRegistered {
    pub campaign: Pubkey,
    pub creator: Pubkey,
    pub tier: u8,
}

#[event]
pub struct ContentSubmitted {
    pub campaign: Pubkey,
    pub creator: Pubkey,
    pub content_uri: String,
}

#[event]
pub struct GrantApproved {
    pub campaign: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
    pub quality_score: u8,
}

#[event]
pub struct GrantRejected {
    pub campaign: Pubkey,
    pub creator: Pubkey,
    pub quality_score: u8,
}

#[event]
pub struct GrantDisbursed {
    pub campaign: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ClawbackExecuted {
    pub campaign: Pubkey,
    pub amount: u64,
}

#[error_code]
pub enum TravelBoostError {
    #[msg("String exceeds maximum allowed capacity.")]
    StringTooLong,
    #[msg("Deadline must be set in the future.")]
    InvalidDeadline,
    #[msg("Invalid tier index specified (must be 0, 1, or 2).")]
    InvalidTier,
    #[msg("Invalid recipient count.")]
    InvalidRecipientCount,
    #[msg("Amount must be strictly greater than zero.")]
    InvalidAmount,
    #[msg("Campaign deadline has passed.")]
    CampaignExpired,
    #[msg("Application is in an invalid state for this operation.")]
    InvalidState,
    #[msg("Quality score out of bounds (must be 0 - 100).")]
    InvalidScore,
    #[msg("Maximum campaign grant recipients reached.")]
    MaxRecipientsReached,
    #[msg("Insufficient escrow vault balance for this tier.")]
    InsufficientVaultFunds,
    #[msg("Approved grant amount cannot be zero.")]
    ZeroGrantAmount,
    #[msg("Campaign is still active; deadline has not arrived.")]
    CampaignNotExpired,
    #[msg("Escrow vault has zero balance.")]
    ZeroVaultBalance,
    #[msg("Mathematical calculation overflowed.")]
    MathOverflow,
}