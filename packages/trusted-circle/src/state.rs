use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw0::Expiration;
use cw3::{Status, Vote};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Dso {
    pub name: String,
    pub escrow_amount: Uint128,
    pub escrow_pending: Option<PendingEscrow>,
    pub rules: VotingRules,
}

/// Pending escrow
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct PendingEscrow {
    /// Associated proposal_id
    pub proposal_id: u64,
    /// Pending escrow amount
    pub amount: Uint128,
    /// Timestamp (seconds) when the pending escrow is enforced
    pub grace_ends_at: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, JsonSchema)]
pub struct VotingRules {
    /// Length of voting period in days.
    /// Also used to define when escrow_pending is enforced.
    pub voting_period: u32,
    /// quorum requirement (0.0-1.0)
    pub quorum: Decimal,
    /// threshold requirement (0.5-1.0)
    pub threshold: Decimal,
    /// If true, and absolute threshold and quorum are met, we can end before voting period finished
    pub allow_end_early: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, JsonSchema)]
pub struct DsoAdjustments {
    /// Escrow name
    pub name: Option<String>,
    /// Escrow amount to apply after grace period (computed using voting_period)
    pub escrow_amount: Option<Uint128>,
    /// Length of voting period in days
    pub voting_period: Option<u32>,
    /// quorum requirement (0.0-1.0)
    pub quorum: Option<Decimal>,
    /// threshold requirement (0.5-1.0)
    pub threshold: Option<Decimal>,
    /// If true, and absolute threshold and quorum are met, we can end before voting period finished
    pub allow_end_early: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, JsonSchema)]
pub enum Punishment {
    DistributeEscrow {
        /// Member to slash / expel
        member: String,
        /// Slashing percentage
        slashing_percentage: Decimal,
        /// Distribution list to send member's slashed escrow amount.
        /// If empty (and `burn_tokens` is false), funds are kept in member's escrow.
        /// `slashing_percentage` is irrelevant / ignored in that case
        distribution_list: Vec<String>,
        /// If set to false, slashed member is demoted to `Pending`. Or not demoted at all,
        /// depending on the amount of funds he retains in escrow.
        /// If set to true, slashed member is effectively demoted to `Leaving`
        kick_out: bool,
    },
    BurnEscrow {
        /// Member to slash / expel
        member: String,
        /// Slashing percentage
        slashing_percentage: Decimal,
        /// If set to false, slashed member is demoted to `Pending`. Or not demoted at all,
        /// depending on the amount of funds he retains in escrow.
        /// If set to true, slashed member is effectively demoted to `Leaving`
        kick_out: bool,
    },
}

/// We store escrow and status together for all members.
/// This is set for any address where weight is not None.
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct EscrowStatus {
    /// how much escrow they have paid
    pub paid: Uint128,
    /// voter status. we check this to see what functionality are allowed for this member
    pub status: MemberStatus,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Copy)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatus {
    /// Normal member, not allowed to vote
    NonVoting {},
    /// Approved for voting, need to pay in
    Pending { proposal_id: u64 },
    /// Approved for voting, and paid in. Waiting for rest of batch
    PendingPaid { proposal_id: u64 },
    /// Full-fledged voting member
    Voting {},
    /// Marked as leaving. Escrow frozen until `claim_at`
    Leaving { claim_at: u64 },
}

/// A Batch is a group of members who got voted in together. We need this to
/// calculate moving from *Paid, Pending Voter* to *Voter*
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Batch {
    /// Timestamp (seconds) when all members are no longer pending
    pub grace_ends_at: u64,
    /// How many must still pay in their escrow before the batch is early authorized
    pub waiting_escrow: u32,
    /// All paid members promoted. We do this once when grace ends or waiting escrow hits 0.
    /// Store this one done so we don't loop through that anymore.
    pub batch_promoted: bool,
    /// List of all members that are part of this batch (look up ESCROWS with these keys)
    pub members: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ProposalContent {
    /// Apply a diff to the existing non-voting members.
    /// Remove is applied after add, so if an address is in both, it is removed
    AddRemoveNonVotingMembers {
        remove: Vec<String>,
        add: Vec<String>,
    },
    EditDso(DsoAdjustments),
    AddVotingMembers {
        voters: Vec<String>,
    },
    PunishMembers(Vec<Punishment>),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Proposal {
    pub title: String,
    pub description: String,
    pub start_height: u64,
    pub expires: Expiration,
    pub proposal: ProposalContent,
    pub status: Status,
    /// pass requirements
    pub rules: VotingRules,
    // the total weight when the proposal started (used to calculate percentages)
    pub total_weight: u64,
    // summary of existing votes
    pub votes: Votes,
}

// weight of votes for each option
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Votes {
    pub yes: u64,
    pub no: u64,
    pub abstain: u64,
    pub veto: u64,
}

// we cast a ballot with our chosen vote and a given weight
// stored under the key that voted
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Ballot {
    pub weight: u64,
    pub vote: Vote,
}
