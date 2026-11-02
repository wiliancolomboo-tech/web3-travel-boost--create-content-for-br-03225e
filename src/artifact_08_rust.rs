use anchor_lang::prelude::*;
use crate::state::{Campaign, ContentSubmission};
use crate::errors::TravelBoostError;

#[derive(Accounts)]
#[instruction(content_hash: [u8; 32])]
pub struct RegisterContent<'info> {
    #[account(
        mut,
        has_one = is_active @ TravelBoostError::CampaignInactive,
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init,
        payer = creator,
        space = ContentSubmission::LEN,
        seeds = [b"submission", campaign.key().as_ref(), content_hash.as_ref()],
        bump
    )]
    pub submission: Account<'info, ContentSubmission>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<RegisterContent>,
    content_hash: [u8; 32],
    metadata_uri: String,
    grant_tier: u8,
) -> Result<()> {
    let clock = Clock::get()?;
    let campaign = &ctx.accounts.campaign;

    require!(clock.unix_timestamp <= campaign.submission_deadline, TravelBoostError::CampaignWindowClosed);
    require!(metadata_uri.len() <= ContentSubmission::MAX_URI_LEN, TravelBoostError::ContentAlreadyRegistered);

    let submission = &mut ctx.accounts.submission;
    submission.creator = ctx.accounts.creator.key();
    submission.campaign = campaign.key();
    submission.content_hash = content_hash;
    submission.metadata_uri = metadata_uri.clone();
    submission.milestones_approved = [false; 3];
    submission.milestones_claimed = [false; 3];
    submission.grant_tier = grant_tier;
    submission.submitted_at = clock.unix_timestamp;
    submission.bump = ctx.bumps.submission;

    msg!("Content submission registered for Breakpoint 2026. Creator: {}, URI: {}", submission.creator, metadata_uri);
    Ok(())
}