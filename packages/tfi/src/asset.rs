use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::querier::{query_balance, query_token_balance};
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, MessageInfo, QuerierWrapper, StdError,
    StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.info)
    }
}

impl Asset {
    pub fn is_native_token(&self) -> bool {
        self.info.is_native_token()
    }

    pub fn into_msg(self, recipient: Addr) -> StdResult<CosmosMsg> {
        let amount = self.amount;

        match &self.info {
            AssetInfo::Token(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: recipient.to_string(),
                    amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::Native(_) => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: recipient.to_string(),
                amount: vec![self.to_coin()?],
            })),
        }
    }

    pub fn to_coin(&self) -> StdResult<Coin> {
        match &self.info {
            AssetInfo::Native(denom) => Ok(Coin {
                denom: denom.clone(),
                amount: self.amount,
            }),
            _ => Err(StdError::generic_err(
                "cannot convert cw20 asset to native Coin",
            )),
        }
    }

    pub fn assert_sent_native_token_balance(&self, message_info: &MessageInfo) -> StdResult<()> {
        if let AssetInfo::Native(denom) = &self.info {
            match message_info.funds.iter().find(|x| x.denom == *denom) {
                Some(coin) => {
                    if self.amount == coin.amount {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
                None => {
                    if self.amount.is_zero() {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
            }
        } else {
            Ok(())
        }
    }
}

/// AssetInfo contract_addr is usually passed from the cw20 hook
/// so we can trust the contract_addr is properly validated.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token(Addr),
    Native(String),
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssetInfo::Native(denom) => write!(f, "{}", denom),
            AssetInfo::Token(contract_addr) => write!(f, "{}", contract_addr),
        }
    }
}

impl AssetInfo {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            AssetInfo::Native(denom) => denom.as_bytes(),
            AssetInfo::Token(contract_addr) => contract_addr.as_str().as_bytes(),
        }
    }

    pub fn is_native_token(&self) -> bool {
        match self {
            AssetInfo::Native(_) => true,
            AssetInfo::Token(_) => false,
        }
    }
    pub fn query_pool(&self, querier: &QuerierWrapper, pool_addr: Addr) -> StdResult<Uint128> {
        match self {
            AssetInfo::Token(contract_addr) => {
                query_token_balance(querier, contract_addr.clone(), pool_addr)
            }
            AssetInfo::Native(denom) => query_balance(querier, pool_addr, denom.to_string()),
        }
    }

    pub fn equal(&self, asset: &AssetInfo) -> bool {
        match self {
            AssetInfo::Token(contract_addr) => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfo::Token(contract_addr) => self_contract_addr == contract_addr,
                    AssetInfo::Native(_) => false,
                }
            }
            AssetInfo::Native(denom) => {
                let self_denom = denom;
                match asset {
                    AssetInfo::Token(_) => false,
                    AssetInfo::Native(denom) => self_denom == denom,
                }
            }
        }
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[non_exhaustive]
pub struct PairInfo {
    pub asset_infos: [AssetInfo; 2],
    pub contract_addr: Addr,
    pub liquidity_token: Addr,
    #[serde(default = "default_commission")]
    pub commission: Decimal,
}

impl PairInfo {
    pub fn new(asset_infos: [AssetInfo; 2], contract_addr: Addr, liquidity_token: Addr) -> Self {
        Self {
            asset_infos,
            contract_addr,
            liquidity_token,
            commission: default_commission(),
        }
    }

    pub fn with_commission(mut self, commission: Decimal) -> Self {
        self.commission = commission;
        self
    }

    pub fn query_pools(
        &self,
        querier: &QuerierWrapper,
        contract_addr: Addr,
    ) -> StdResult<[Asset; 2]> {
        let info_0 = self.asset_infos[0].clone();
        let info_1 = self.asset_infos[1].clone();
        Ok([
            Asset {
                amount: info_0.query_pool(querier, contract_addr.clone())?,
                info: info_0,
            },
            Asset {
                amount: info_1.query_pool(querier, contract_addr)?,
                info: info_1,
            },
        ])
    }
}

pub(crate) fn default_commission() -> Decimal {
    Decimal::permille(3)
}
