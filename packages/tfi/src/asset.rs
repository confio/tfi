use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Coin, Uint128};
use cw20::{Balance, Cw20CoinVerified};

pub type Asset = Balance;
pub type AssetInfo = Denom;

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInfo {
    pub asset_infos: [Denom; 2],
    pub contract_addr: Addr,
    pub liquidity_token: Addr,
}

// /// AssetInfo contract_addr is usually passed from the cw20 hook
// /// so we can trust the contract_addr is properly validated.
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// pub enum AssetInfo {
//     Token { contract_addr: Addr },
//     NativeToken { denom: String },
// }
//
// impl fmt::Display for AssetInfo {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             AssetInfo::NativeToken { denom } => write!(f, "{}", denom),
//             AssetInfo::Token { contract_addr } => write!(f, "{}", contract_addr),
//         }
//     }
// }
//
// impl AssetInfo {
//     pub fn to_raw(&self, api: &dyn Api) -> StdResult<AssetInfoRaw> {
//         match self {
//             AssetInfo::NativeToken { denom } => Ok(AssetInfoRaw::NativeToken {
//                 denom: denom.to_string(),
//             }),
//             AssetInfo::Token { contract_addr } => Ok(AssetInfoRaw::Token {
//                 contract_addr: api.addr_canonicalize(contract_addr.as_str())?,
//             }),
//         }
//     }
//
//     pub fn is_native_token(&self) -> bool {
//         match self {
//             AssetInfo::NativeToken { .. } => true,
//             AssetInfo::Token { .. } => false,
//         }
//     }
//     pub fn query_pool(&self, querier: &QuerierWrapper, pool_addr: Addr) -> StdResult<Uint128> {
//         match self {
//             AssetInfo::Token { contract_addr, .. } => {
//                 query_token_balance(querier, contract_addr.clone(), pool_addr)
//             }
//             AssetInfo::NativeToken { denom, .. } => {
//                 query_balance(querier, pool_addr, denom.to_string())
//             }
//         }
//     }
//
//     pub fn equal(&self, asset: &AssetInfo) -> bool {
//         match self {
//             AssetInfo::Token { contract_addr, .. } => {
//                 let self_contract_addr = contract_addr;
//                 match asset {
//                     AssetInfo::Token { contract_addr, .. } => self_contract_addr == contract_addr,
//                     AssetInfo::NativeToken { .. } => false,
//                 }
//             }
//             AssetInfo::NativeToken { denom, .. } => {
//                 let self_denom = denom;
//                 match asset {
//                     AssetInfo::Token { .. } => false,
//                     AssetInfo::NativeToken { denom, .. } => self_denom == denom,
//                 }
//             }
//         }
//     }
// }

/******* TODO: move all this back into cosmwasm-plus : cw20 when needs are met *********/

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Denom {
    Native(String),
    Cw20(Addr),
}

impl Denom {
    pub fn is_empty(&self) -> bool {
        match self {
            Denom::Native(string) => string.is_empty(),
            Denom::Cw20(addr) => addr.as_ref().is_empty(),
        }
    }

    pub fn with_amount(&self, amount: Uint128) -> Balance {
        match self {
            Self::Native(denom) => vec![Coin {
                denom: denom.to_string(),
                amount,
            }]
            .into(),
            Self::Cw20(addr) => Cw20CoinVerified {
                address: addr.clone(),
                amount,
            }
            .into(),
        }
    }
}
