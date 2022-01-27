use cosmwasm_std::{Decimal, OverflowError, StdError};
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

    #[error("Max spread exceeded, spread ratio: {spread_ratio}, max spread: {max_spread}")]
    MaxSpreadAssertion {
        spread_ratio: Decimal,
        max_spread: Decimal,
    },

    #[error("Max slippage exceeded, deposits ratio: {deposits_ratio}, pools ratio: {pools_ratio}, slippage tolerance: {slippage_tolerance}")]
    MaxSlippageAssertion {
        deposits_ratio: Decimal,
        pools_ratio: Decimal,
        slippage_tolerance: Decimal,
    },

    #[error("Asset mismatch: {0}")]
    AssetMismatch(String),

    #[error("Explicit failure in message: {0}")]
    MessageFailure(String),

    #[error("Missing required data")]
    MissingData {},

    #[error("Invalid address length: {0}")]
    InvalidAddressLength(usize),

    #[error("Invalid commission value: {0}")]
    InvalidCommission(Decimal),
}
