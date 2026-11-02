use anchor_lang::prelude::*;
use crate::state::Campaign;
use crate::errors::TravelBoostError;

#[derive(Accounts)]
#[instruction(campaign_id: [u8; 16])]
pub struct InitializeCampaign<'info> {
    #[account(
        init,
        payer = authority,
        space = Campaign::LEN,
        seeds = [b"campaign", campaign_id.as_ref()],
        bump
    )]
    pub campaign: Account<'info, Campaign>,

    /// CHECK: PDA acting as the fund vault
    #[account(
        mut,
        seeds = [b"vault", campaign.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    /// Oracle/attestor pubkey
    /// CHECK: Safe validation
    pub validator_oracle: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeCampaign>,
    campaign_id: [u8; 16],
    total_budget: u64,
    submission_deadline: i64,
) -> Result<()> {
    let clock = Clock::get()?;
    require!(submission_deadline > clock.unix_timestamp, TravelBoostError::CampaignWindowClosed);

    let campaign = &mut ctx.accounts.campaign;
    campaign.authority = ctx.accounts.authority.key();
    campaign.validator_oracle = ctx.accounts.validator_oracle.key();
    campaign.vault = ctx.accounts.vault.key();
    campaign.total_budget = total_budget;
    campaign.disbursed_funds = 0;
    campaign.start_time = clock.unix_timestamp;
    campaign.submission_deadline = submission_deadline;
    campaign.is_active = true;
    campaign.campaign_id = campaign_id;
    campaign.bump = ctx.bumps.campaign;

    msg!("Breakpoint 2026 TravelBoost Campaign initialized: {:?}", campaign_id);
    Ok(())
}