use cosmwasm_std::StdError;
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
}
