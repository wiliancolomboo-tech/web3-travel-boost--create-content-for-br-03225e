use anchor_lang::prelude::*;

#[account]
#[derive(Default)]
pub struct Campaign {
    /// Authority who initialized the campaign (e.g. Breakpoint Committee)
    pub authority: Pubkey,
    /// Designated oracle/committee key for milestone attestations
    pub validator_oracle: Pubkey,
    /// Vault account holding native lamports or SPL USDC
    pub vault: Pubkey,
    /// Total travel fund budget in lamports
    pub total_budget: u64,
    /// Total funds disbursed to date
    pub disbursed_funds: u64,
    /// Campaign start timestamp
    pub start_time: i64,
    /// Deadline for content creation submissions
    pub submission_deadline: i64,
    /// Active state flag
    pub is_active: bool,
    /// Campaign identifier seed
    pub campaign_id: [u8; 16],
    /// PDA bump
    pub bump: u8,
}

impl Campaign {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        32 + // validator_oracle
        32 + // vault
        8 +  // total_budget
        8 +  // disbursed_funds
        8 +  // start_time
        8 +  // submission_deadline
        1 +  // is_active
        16 + // campaign_id
        1;   // bump
}

#[account]
#[derive(Default)]
pub struct ContentSubmission {
    /// Creator wallet address
    pub creator: Pubkey,
    /// Reference to the campaign account
    pub campaign: Pubkey,
    /// SHA-256 hash of the content payload (URL, GitHub commit, or technical piece)
    pub content_hash: [u8; 32],
    /// URI pointing to technical content metadata (IPFS or Arweave)
    pub metadata_uri: String,
    /// Milestone approval status: Tier 1 (Research), Tier 2 (Artifact), Tier 3 (Viral/Final)
    pub milestones_approved: [bool; 3],
    /// Milestone disbursement claim status
    pub milestones_claimed: [bool; 3],
    /// Grant tier assigned (1 = Bronze $500, 2 = Silver $1500, 3 = Gold $3000 Breakpoint package)
    pub grant_tier: u8,
    /// Timestamp of submission
    pub submitted_at: i64,
    /// PDA bump
    pub bump: u8,
}

impl ContentSubmission {
    pub const MAX_URI_LEN: usize = 128;
    pub const LEN: usize = 8 + // discriminator
        32 + // creator
        32 + // campaign
        32 + // content_hash
        (4 + Self::MAX_URI_LEN) + // metadata_uri string
        3 +  // milestones_approved [bool; 3]
        3 +  // milestones_claimed [bool; 3]
        1 +  // grant_tier
        8 +  // submitted_at
        1;   // bump
}