use anchor_lang::prelude::*;
use crate::state::{Campaign, ContentSubmission};
use crate::errors::TravelBoostError;

#[derive(Accounts)]
pub struct AttestMilestone<'info> {
    #[account(
        has_one = validator_oracle @ TravelBoostError::UnauthorizedValidator,
        has_one = is_active @ TravelBoostError::CampaignInactive,
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        has_one = campaign @ TravelBoostError::UnauthorizedAuthority
    )]
    pub submission: Account<'info, ContentSubmission>,

    pub validator_oracle: Signer<'info>,
}

pub fn handler(
    ctx: Context<AttestMilestone>,
    milestone_index: u8,
) -> Result<()> {
    let idx = milestone_index as usize;
    require!(idx < 3, TravelBoostError::InvalidMilestoneIndex);

    let submission = &mut ctx.accounts.submission;
    require!(!submission.milestones_approved[idx], TravelBoostError::MilestoneAlreadyApproved);

    submission.milestones_approved[idx] = true;

    msg!(
        "Milestone {} attested by Oracle for creator: {}",
        milestone_index,
        submission.creator
    );
    Ok(())
}