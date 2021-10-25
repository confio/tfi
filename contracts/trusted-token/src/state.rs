use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw4::Cw4Contract;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WHITELIST: Item<Cw4Contract> = Item::new("whitelist");
pub const REEDEMS: Map<&str, Redeem> = Map::new("redeems");

/// Entry about redeem which had place
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Redeem {
    /// Sender which triggered redeem
    pub sender: Addr,
    /// Amount of redeemed tokens
    pub amount: Uint128,
    /// Memo embeded in redeem message
    pub memo: String,
    /// Timestampt when redeem took place
    pub timestamp: Timestamp,
}
