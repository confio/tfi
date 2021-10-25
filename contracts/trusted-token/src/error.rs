use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Cw20(#[from] cw20_base::ContractError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Redeem code already used")]
    RedeemCodeUsed {},

    #[error("Trying to reedem more funds than account balance, {0} tokens available")]
    RedeemOverBalance(Uint128),
}

impl From<std::str::Utf8Error> for ContractError {
    fn from(source: std::str::Utf8Error) -> Self {
        Self::Std(source.into())
    }
}
