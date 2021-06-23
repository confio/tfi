use cw_storage_plus::Item;
use tfi::asset::PairInfo;

// put the length bytes at the first for compatibility with legacy singleton store
pub const PAIR_INFO: Item<PairInfo> = Item::new("pair_info");
