use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("TrvL8o26BreakpointBoost11111111111111111111");

#[program]
pub mod travel_boost {
    use super::*;

    /// Initializes a new travel boost grant campaign funded by a sponsor.
    pub fn initialize_grant(
        ctx: Context<InitializeGrant>,
        grant_id: u64,
        amount: u64,
        deadline: i64,
        title: String,
        content_rubric: String,
    ) -> Result<()> {
        require!(amount > 0, TravelBoostError::InvalidAmount);
        require!(
            deadline > Clock::get()?.unix_timestamp,
            TravelBoostError::DeadlineMustBeFuture
        );
        require!(
            title.len() <= 64,
            TravelBoostError::TitleExceedsMaxLength
        );
        require!(
            content_rubric.len() <= 256,
            TravelBoostError::RubricExceedsMaxLength
        );

        let grant = &mut ctx.accounts.grant_account;
        grant.sponsor = ctx.accounts.sponsor.key();
        grant.creator = ctx.accounts.creator.key();
        grant.validator = ctx.accounts.validator.key();
        grant.vault = ctx.accounts.vault.key();
        grant.grant_id = grant_id;
        grant.amount = amount;
        grant.deadline = deadline;
        grant.status = GrantStatus::Funded;
        grant.content_hash = [0u8; 32];
        grant.content_uri = String::default();
        grant.bump = ctx.bumps.grant_account;
        grant.vault_bump = ctx.bumps.vault;

        // Transfer funds from sponsor into the PDA Vault
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.sponsor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi_context, amount)?;

        emit!(GrantInitialized {
            grant_id,
            sponsor: grant.sponsor,
            creator: grant.creator,
            amount,
            deadline,
            title,
        });

        Ok(())
    }

    /// Submits the technical content hash and URI for Breakpoint 2026.
    pub fn submit_content(
        ctx: Context<SubmitContent>,
        content_hash: [u8; 32],
        content_uri: String,
    ) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;

        require!(
            grant.status == GrantStatus::Funded,
            TravelBoostError::InvalidGrantState
        );
        require!(
            Clock::get()?.unix_timestamp <= grant.deadline,
            TravelBoostError::GrantExpired
        );
        require!(
            !content_uri.is_empty() && content_uri.len() <= 200,
            TravelBoostError::InvalidContentUri
        );
        require!(
            content_hash != [0u8; 32],
            TravelBoostError::InvalidContentHash
        );

        grant.content_hash = content_hash;
        grant.content_uri = content_uri.clone();
        grant.status = GrantStatus::Submitted;

        emit!(ContentSubmitted {
            grant_id: grant.grant_id,
            creator: grant.creator,
            content_hash,
            content_uri,
        });

        Ok(())
    }

    /// Verifies content against Breakpoint 2026 technical requirements and disburses travel stipend.
    pub fn verify_and_disburse(ctx: Context<VerifyAndDisburse>) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;

        require!(
            grant.status == GrantStatus::Submitted,
            TravelBoostError::InvalidGrantState
        );
        require!(
            Clock::get()?.unix_timestamp <= grant.deadline,
            TravelBoostError::GrantExpired
        );

        let payout_amount = grant.amount;
        grant.status = GrantStatus::Completed;

        // PDA signer seeds for Vault transfer
        let grant_id_bytes = grant.grant_id.to_le_bytes();
        let seeds: &[&[&[u8]]] = &[&[
            b"vault",
            grant.to_account_info().key.as_ref(),
            &grant_id_bytes,
            &[grant.vault_bump],
        ]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.creator.to_account_info(),
            },
            seeds,
        );
        transfer(cpi_context, payout_amount)?;

        emit!(GrantDisbursed {
            grant_id: grant.grant_id,
            creator: grant.creator,
            amount: payout_amount,
            validator: ctx.accounts.validator.key(),
        });

        Ok(())
    }

    /// Allows sponsor to reclaim stipend capital if creator fails to deliver content before deadline.
    pub fn refund_expired_grant(ctx: Context<RefundExpiredGrant>) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;

        require!(
            grant.status != GrantStatus::Completed,
            TravelBoostError::GrantAlreadyCompleted
        );
        require!(
            Clock::get()?.unix_timestamp > grant.deadline,
            TravelBoostError::DeadlineNotMet
        );

        let refund_amount = ctx.accounts.vault.lamports();
        require!(refund_amount > 0, TravelBoostError::InsufficientVaultFunds);

        grant.status = GrantStatus::Refunded;

        let grant_id_bytes = grant.grant_id.to_le_bytes();
        let seeds: &[&[&[u8]]] = &[&[
            b"vault",
            grant.to_account_info().key.as_ref(),
            &grant_id_bytes,
            &[grant.vault_bump],
        ]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.sponsor.to_account_info(),
            },
            seeds,
        );
        transfer(cpi_context, refund_amount)?;

        emit!(GrantRefunded {
            grant_id: grant.grant_id,
            sponsor: grant.sponsor,
            amount_refunded: refund_amount,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION STRUCTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(grant_id: u64)]
pub struct InitializeGrant<'info> {
    #[account(
        init,
        payer = sponsor,
        space = 8 + GrantAccount::INIT_SPACE,
        seeds = [b"grant", sponsor.key().as_ref(), &grant_id.to_le_bytes()],
        bump
    )]
    pub grant_account: Account<'info, GrantAccount>,

    #[account(
        mut,
        seeds = [b"vault", grant_account.key().as_ref(), &grant_id.to_le_bytes()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// CHECK: Creator receiving the travel boost grant
    pub creator: AccountInfo<'info>,

    /// CHECK: Authorized ecosystem technical validator
    pub validator: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitContent<'info> {
    #[account(
        mut,
        has_one = creator @ TravelBoostError::UnauthorizedCreator,
        seeds = [b"grant", grant_account.sponsor.as_ref(), &grant_account.grant_id.to_le_bytes()],
        bump = grant_account.bump
    )]
    pub grant_account: Account<'info, GrantAccount>,

    pub creator: Signer<'info>,
}

#[derive(Accounts)]
pub struct VerifyAndDisburse<'info> {
    #[account(
        mut,
        has_one = validator @ TravelBoostError::UnauthorizedValidator,
        has_one = creator,
        has_one = vault,
        seeds = [b"grant", grant_account.sponsor.as_ref(), &grant_account.grant_id.to_le_bytes()],
        bump = grant_account.bump
    )]
    pub grant_account: Account<'info, GrantAccount>,

    #[account(
        mut,
        seeds = [b"vault", grant_account.key().as_ref(), &grant_account.grant_id.to_le_bytes()],
        bump = grant_account.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    /// CHECK: Verified creator recipient
    #[account(mut)]
    pub creator: AccountInfo<'info>,

    pub validator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RefundExpiredGrant<'info> {
    #[account(
        mut,
        has_one = sponsor @ TravelBoostError::UnauthorizedSponsor,
        has_one = vault,
        seeds = [b"grant", grant_account.sponsor.as_ref(), &grant_account.grant_id.to_le_bytes()],
        bump = grant_account.bump
    )]
    pub grant_account: Account<'info, GrantAccount>,

    #[account(
        mut,
        seeds = [b"vault", grant_account.key().as_ref(), &grant_account.grant_id.to_le_bytes()],
        bump = grant_account.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// -----------------------------------------------------------------------------
// STATE DEFINITIONS
// -----------------------------------------------------------------------------

#[account]
#[derive(InitSpace)]
pub struct GrantAccount {
    pub sponsor: Pubkey,
    pub creator: Pubkey,
    pub validator: Pubkey,
    pub vault: Pubkey,
    pub grant_id: u64,
    pub amount: u64,
    pub deadline: i64,
    pub status: GrantStatus,
    pub content_hash: [u8; 32],
    #[max_len(200)]
    pub content_uri: String,
    pub bump: u8,
    pub vault_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum GrantStatus {
    Funded,
    Submitted,
    Completed,
    Refunded,
}

// -----------------------------------------------------------------------------
// EVENTS & ERRORS
// -----------------------------------------------------------------------------

#[event]
pub struct GrantInitialized {
    pub grant_id: u64,
    pub sponsor: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
    pub deadline: i64,
    pub title: String,
}

#[event]
pub struct ContentSubmitted {
    pub grant_id: u64,
    pub creator: Pubkey,
    pub content_hash: [u8; 32],
    pub content_uri: String,
}

#[event]
pub struct GrantDisbursed {
    pub grant_id: u64,
    pub creator: Pubkey,
    pub amount: u64,
    pub validator: Pubkey,
}

#[event]
pub struct GrantRefunded {
    pub grant_id: u64,
    pub sponsor: Pubkey,
    pub amount_refunded: u64,
}

#[error_code]
pub enum TravelBoostError {
    #[msg("Grant amount must be strictly greater than zero.")]
    InvalidAmount,
    #[msg("Grant expiration deadline must be in the future.")]
    DeadlineMustBeFuture,
    #[msg("Grant title exceeds the maximum 64 character length.")]
    TitleExceedsMaxLength,
    #[msg("Grant rubric exceeds the maximum 256 character length.")]
    RubricExceedsMaxLength,
    #[msg("Action cannot be performed under the current grant lifecycle state.")]
    InvalidGrantState,
    #[msg("The grant deadline has passed.")]
    GrantExpired,
    #[msg("The provided content URI is empty or exceeds 200 characters.")]
    InvalidContentUri,
    #[msg("Invalid SHA256 content verification hash provided.")]
    InvalidContentHash,
    #[msg("Only the designated creator can submit content for this grant.")]
    UnauthorizedCreator,
    #[msg("Only the designated validator can verify content for this grant.")]
    UnauthorizedValidator,
    #[msg("Only the sponsor can request a refund for this grant.")]
    UnauthorizedSponsor,
    #[msg("Cannot refund a grant that has already been completed.")]
    GrantAlreadyCompleted,
    #[msg("Cannot refund a grant prior to the expiration deadline.")]
    DeadlineNotMet,
    #[msg("Insufficient lamports remaining in the grant vault.")]
    InsufficientVaultFunds,
}