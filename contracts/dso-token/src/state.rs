use cw4::Cw4Contract;
use cw_storage_plus::Item;

pub const WHITELIST: Item<Cw4Contract> = Item::new("whitelist");
