use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{to_binary, Addr, BankMsg, Coin, CosmosMsg, StdResult, Uint128, WasmMsg};
use cw20::{Balance, Cw20CoinVerified, Cw20ExecuteMsg};
use std::fmt;
use std::fmt::Formatter;

pub type Asset = Balance;
pub type AssetInfo = Denom;

pub fn send_asset(asset: Asset, to_addr: Addr) -> StdResult<CosmosMsg> {
    match asset {
        Balance::Native(bal) => Ok(BankMsg::Send {
            to_address: to_addr.into(),
            amount: bal.0,
        }
        .into()),
        Balance::Cw20(cw20) => Ok(WasmMsg::Execute {
            contract_addr: cw20.address.into(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: to_addr.into(),
                amount: cw20.amount,
            })?,
            send: vec![],
        }
        .into()),
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInfo {
    pub asset_infos: [Denom; 2],
    pub contract_addr: Addr,
    pub liquidity_token: Addr,
}

//
// impl AssetInfo {
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
// }

/******* TODO: move all this back into cosmwasm-plus : cw20 when needs are met *********/

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Denom {
    Native(String),
    Cw20(Addr),
}

impl fmt::Display for Denom {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(denom) => write!(f, "{}", denom),
            Self::Cw20(addr) => write!(f, "{}", addr),
        }
    }
}

impl Denom {
    pub fn is_native_token(&self) -> bool {
        matches!(self, Denom::Native(_))
    }

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
