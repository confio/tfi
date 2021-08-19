use cosmwasm_std::{Addr, Binary, Timestamp, Uint128};
use cw20::{Cw20Coin, Expiration, Logo, MinterResponse};
use cw20_base::msg::InstantiateMarketingInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::Reedem;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<Cw20Coin>,
    pub mint: Option<MinterResponse>,
    pub marketing: Option<InstantiateMarketingInfo>,
    /// This is the address of a cw4 compatible contract that will serve as a whitelist
    pub whitelist_group: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Transfer is a base message to move tokens to another account without triggering actions
    Transfer { recipient: String, amount: Uint128 },
    /// Burn is a base message to destroy tokens forever
    Burn { amount: Uint128 },
    /// Send is a base message to transfer tokens to a contract and trigger an action
    /// on the receiving contract.
    Send {
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Only with "approval" extension. Allows spender to access an additional amount tokens
    /// from the owner's (env.sender) account. If expires is Some(), overwrites current allowance
    /// expiration with this one.
    IncreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Only with "approval" extension. Lowers the spender's access of tokens
    /// from the owner's (env.sender) account by amount. If expires is Some(), overwrites current
    /// allowance expiration with this one.
    DecreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Only with "approval" extension. Transfers amount tokens from owner -> recipient
    /// if `env.sender` has sufficient pre-approval.
    TransferFrom {
        owner: String,
        recipient: String,
        amount: Uint128,
    },
    /// Only with "approval" extension. Sends amount tokens from owner -> contract
    /// if `env.sender` has sufficient pre-approval.
    SendFrom {
        owner: String,
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Only with "approval" extension. Destroys tokens forever
    BurnFrom { owner: String, amount: Uint128 },
    /// Only with the "mintable" extension. If authorized, creates amount new tokens
    /// and adds to the recipient balance.
    Mint { recipient: String, amount: Uint128 },
    /// Only with the "marketing" extension. If authorized, updates marketing metadata.
    /// Setting None/null for any of these will leave it unchanged.
    /// Setting Some("") will clear this field on the contract storage
    UpdateMarketing {
        /// A URL pointing to the project behind this token.
        project: Option<String>,
        /// A longer description of the token and it's utility. Designed for tooltips or such
        description: Option<String>,
        /// The address (if any) who can update this data structure
        marketing: Option<String>,
    },
    /// If set as the "marketing" role on the contract, upload a new URL, SVG, or PNG for the token
    UploadLogo(Logo),

    // Non-standard messages
    /// Reedems tokens
    ///
    /// Before calling this, there should be agreement with token provider, that equivalent is
    /// covered offchain, otherwise this is just an equivalent of burning own tokens.
    ///
    /// This causes `reedem` event which token admin may subscribe to to finalize reedem process.
    /// It also stores all reedems internally so it can be queried to check for reedems to be
    /// finalized.
    Reedem {
        /// Amount of tokens to be reedemed
        amount: Uint128,
        /// Reedem code agreed with token owner
        code: String,
        /// Account on behalf which reedem is performed, if not set message sender is presumed
        sender: Option<String>,
        /// Meta information about reedem
        memo: String,
    },
    /// Removes information about reedems. Only minter may perform this, as he is
    /// the one responsible for reedeming actions.
    RemoveReedems {
        /// Reedem codes to be removed
        codes: Vec<String>,
    },
    /// Removes all reedems informations. Only minter may perform this.
    ClearReedems {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the cw4 contract used to whitelist this token.
    /// Return type: WhitelistResponse
    Whitelist {},
    /// Returns true if the address is in the Whitelist contract.
    /// Just a helper around querying the whitelist, then querying those members
    IsWhitelisted { address: String },
    /// Returns the current balance of the given address, 0 if unset.
    /// Return type: BalanceResponse.
    Balance { address: String },
    /// Returns metadata on the contract - name, decimals, supply, etc.
    /// Return type: TokenInfoResponse.
    TokenInfo {},
    /// Only with "mintable" extension.
    /// Returns who can mint and how much.
    /// Return type: MinterResponse.
    Minter {},
    /// Only with "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    /// Return type: AllowanceResponse.
    Allowance { owner: String, spender: String },
    /// Only with "enumerable" extension (and "allowances")
    /// Returns all allowances this owner has approved. Supports pagination.
    /// Return type: AllAllowancesResponse.
    AllAllowances {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Only with "enumerable" extension
    /// Returns all accounts that have balances. Supports pagination.
    /// Return type: AllAccountsResponse.
    AllAccounts {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Only with "marketing" extension
    /// Returns more metadata on the contract to display in the client:
    /// - description, logo, project url, etc.
    /// Return type: MarketingInfoResponse
    MarketingInfo {},
    /// Only with "marketing" extension
    /// Downloads the embedded logo data (if stored on chain). Errors if no logo data stored for this
    /// contract.
    /// Return type: DownloadLogoResponse.
    DownloadLogo {},
    /// Get info about particular reedem
    ///
    /// Return type: ReedemResponse
    Reedem {
        /// Code used for reedem
        code: String,
    },
    /// Returns reedems which took place on this token
    /// Return type: AllReedemsResponse
    AllReedems {
        /// Reedem code where to start reading for pagination
        start_after: Option<String>,
        /// Maximum number of entries to return
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WhitelistResponse {
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IsWhitelistedResponse {
    pub whitelisted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ReedemInfo {
    /// Code used for this reedem
    pub code: String,
    /// Sender which triggered reedem
    pub sender: Addr,
    /// Amount of reedemed tokens
    pub amount: Uint128,
    /// Memo embeded in reedem message
    pub memo: String,
    /// Timestampt when reedem took place
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AllReedemsResponse {
    pub reedems: Vec<ReedemInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ReedemResponse {
    pub reedem: Option<Reedem>,
}
