use anchor_lang::prelude::*;
use crate::state::{Campaign, ContentSubmission};
use crate::errors::TravelBoostError;

#[derive(Accounts)]
pub struct ClaimTravelGrant<'info> {
    #[account(mut)]
    pub campaign: Account<'info, Campaign>,

    /// Escrow PDA holding campaign funds
    /// CHECK: Verified via seed derivation
    #[account(
        mut,
        seeds = [b"vault", campaign.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        has_one = campaign @ TravelBoostError::UnauthorizedAuthority,
        has_one = creator @ TravelBoostError::UnauthorizedAuthority
    )]
    pub submission: Account<'info, ContentSubmission>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ClaimTravelGrant>,
    milestone_index: u8,
    disbursement_lamports: u64,
) -> Result<()> {
    let idx = milestone_index as usize;
    require!(idx < 3, TravelBoostError::InvalidMilestoneIndex);

    let submission = &mut ctx.accounts.submission;
    require!(submission.milestones_approved[idx], TravelBoostError::MilestoneUnfulfilled);
    require!(!submission.milestones_claimed[idx], TravelBoostError::MilestoneAlreadyApproved);

    let campaign = &mut ctx.accounts.campaign;
    require!(
        ctx.accounts.vault.lamports() >= disbursement_lamports,
        TravelBoostError::InsufficientEscrowFunds
    );

    // Transfer native SOL lamports from PDA vault to Creator
    let campaign_key = campaign.key();
    let (_, vault_bump) = Pubkey::find_program_address(
        &[b"vault", campaign_key.as_ref()],
        ctx.program_id
    );
    let seeds = &[
        b"vault",
        campaign_key.as_ref(),
        &[vault_bump],
    ];
    let signer_seeds = &[&seeds[..]];

    anchor_lang::solana_program::program::invoke_signed(
        &anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.vault.key(),
            &ctx.accounts.creator.key(),
            disbursement_lamports,
        ),
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.creator.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    // Mark milestone claimed
    submission.milestones_claimed[idx] = true;
    campaign.disbursed_funds = campaign
        .disbursed_funds
        .checked_add(disbursement_lamports)
        .ok_or(TravelBoostError::NumericalOverflow)?;

    msg!(
        "Claim successful: {} lamports disbursed to {} for Milestone {}",
        disbursement_lamports,
        ctx.accounts.creator.key(),
        milestone_index
    );
    Ok(())
}