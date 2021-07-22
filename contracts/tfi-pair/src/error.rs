use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Max spread assertion")]
    MaxSpreadAssertion {},

    #[error("Max slippage assertion")]
    MaxSlippageAssertion {},

    #[error("Asset mismatch")]
    AssetMismatch {},

    #[error("Explicit failure in message: {0}")]
    MessageFailure(String),

    #[error("Missing required data")]
    MissingData {},

    #[error("Invalid address length: {0}")]
    InvalidAddressLength(usize),
}
