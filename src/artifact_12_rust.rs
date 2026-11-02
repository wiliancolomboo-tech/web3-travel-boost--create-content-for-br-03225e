use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("TrvL86V7K4qE5q9sA5tW4bMvUeB2NqV5cZ7h8mP1r9d");

#[program]
pub mod travel_boost {
    use super::*;

    pub fn initialize_campaign(
        ctx: Context<InitializeCampaign>,
        campaign_id: [u8; 16],
        total_budget: u64,
        submission_deadline: i64,
    ) -> Result<()> {
        instructions::initialize_campaign::handler(ctx, campaign_id, total_budget, submission_deadline)
    }

    pub fn register_content(
        ctx: Context<RegisterContent>,
        content_hash: [u8; 32],
        metadata_uri: String,
        grant_tier: u8,
    ) -> Result<()> {
        instructions::register_content::handler(ctx, content_hash, metadata_uri, grant_tier)
    }

    pub fn attest_milestone(
        ctx: Context<AttestMilestone>,
        milestone_index: u8,
    ) -> Result<()> {
        instructions::attest_milestone::handler(ctx, milestone_index)
    }

    pub fn claim_travel_grant(
        ctx: Context<ClaimTravelGrant>,
        milestone_index: u8,
        disbursement_lamports: u64,
    ) -> Result<()> {
        instructions::claim_travel_grant::handler(ctx, milestone_index, disbursement_lamports)
    }
}