use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw4::Cw4Contract;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WHITELIST: Item<Cw4Contract> = Item::new("whitelist");
pub const REEDEMS: Map<String, Reedem> = Map::new("reedems");

/// Entry about reedem which had place
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Reedem {
    /// Sender which triggered reedem
    pub sender: Addr,
    /// Amount of reedemed tokens
    pub amount: Uint128,
    /// Memo embeded in reedem message
    pub memo: String,
    /// Timestampt when reedem took place
    pub timestamp: Timestamp,
}
