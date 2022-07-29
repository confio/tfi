use cosmwasm_std::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{default_commission, AssetInfo, PairInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[non_exhaustive]
pub struct InstantiateMsg {
    /// Pair contract code ID, which is used to
    pub pair_code_id: u64,
    pub token_code_id: u64,
    /// Default commission to be set on newly created pair, 0.003 by default
    #[serde(default = "default_commission")]
    pub default_commission: Decimal,
}

impl InstantiateMsg {
    pub fn new(pair_code_id: u64, token_code_id: u64) -> Self {
        Self {
            pair_code_id,
            token_code_id,
            default_commission: default_commission(),
        }
    }

    pub fn with_default_commission(mut self, commission: Decimal) -> Self {
        self.default_commission = commission;
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// UpdateConfig update relevant code IDs
    UpdateConfig {
        owner: Option<String>,
        token_code_id: Option<u64>,
        pair_code_id: Option<u64>,
        default_commission: Option<Decimal>,
    },
    /// CreatePair instantiates pair contract
    CreatePair {
        /// Asset infos
        asset_infos: [AssetInfo; 2],
        /// Commission on created pair. If none, default commission from factory configuration would
        /// be used.
        commission: Option<Decimal>,
    },
}

/// Utility for creating `ExecuteMsg::UpdateConfig` variant
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct ExecuteUpdateConfig {
    pub owner: Option<String>,
    pub token_code_id: Option<u64>,
    pub pair_code_id: Option<u64>,
    pub default_commission: Option<Decimal>,
}

impl ExecuteUpdateConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }

    pub fn with_token_code_id(mut self, id: u64) -> Self {
        self.token_code_id = Some(id);
        self
    }

    pub fn with_pair_code_id(mut self, id: u64) -> Self {
        self.pair_code_id = Some(id);
        self
    }

    pub fn with_default_commission(mut self, commission: Decimal) -> Self {
        self.default_commission = Some(commission);
        self
    }
}

impl From<ExecuteUpdateConfig> for ExecuteMsg {
    fn from(src: ExecuteUpdateConfig) -> Self {
        Self::UpdateConfig {
            owner: src.owner,
            token_code_id: src.token_code_id,
            pair_code_id: src.pair_code_id,
            default_commission: src.default_commission,
        }
    }
}

/// Utility for creating `ExecuteMsg::UpdatePair` variant
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ExecuteCreatePair {
    /// Asset infos
    asset_infos: [AssetInfo; 2],
    /// Commision on created pair
    commission: Option<Decimal>,
}

impl ExecuteCreatePair {
    pub fn new(asset_infos: [AssetInfo; 2]) -> Self {
        Self {
            asset_infos,
            commission: None,
        }
    }

    pub fn with_commission(mut self, commission: Decimal) -> Self {
        self.commission = Some(commission);
        self
    }
}

impl From<ExecuteCreatePair> for ExecuteMsg {
    fn from(src: ExecuteCreatePair) -> Self {
        Self::CreatePair {
            asset_infos: src.asset_infos,
            commission: src.commission,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Pair {
        asset_infos: [AssetInfo; 2],
    },
    Pairs {
        start_after: Option<[AssetInfo; 2]>,
        limit: Option<u32>,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub pair_code_id: u64,
    pub token_code_id: u64,
    pub default_commission: Decimal,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairsResponse {
    pub pairs: Vec<PairInfo>,
}
