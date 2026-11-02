use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::Campaign;
use crate::errors::BoostError;

#[derive(Accounts)]
pub struct ClawbackExpired<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority @ BoostError::Unauthorized,
        seeds = [b"boost_campaign", campaign.campaign_id.to_le_bytes().as_ref()],
        bump = campaign.bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        mut,
        seeds = [b"campaign_vault", campaign.key().as_ref()],
        bump = campaign.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = campaign.mint,
        token::authority = authority
    )]
    pub authority_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<ClawbackExpired>) -> Result<()> {
    let campaign = &ctx.accounts.campaign;
    let clock = Clock::get()?;

    // Clawback is only allowed after the content deadline has expired
    require!(clock.unix_timestamp > campaign.content_deadline, BoostError::CampaignNotExpired);

    let remaining_balance = ctx.accounts.vault.amount;
    if remaining_balance == 0 {
        return Ok(());
    }

    let campaign_key = campaign.key();
    let seeds = &[
        b"campaign_vault",
        campaign_key.as_ref(),
        &[campaign.vault_bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_accounts = Transfer {
        from: ctx.accounts.vault.to_account_info(),
        to: ctx.accounts.authority_token_account.to_account_info(),
        authority: ctx.accounts.vault.to_account_info(),
    };
    token::transfer(
        CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer),
        remaining_balance,
    )?;

    Ok(())
}