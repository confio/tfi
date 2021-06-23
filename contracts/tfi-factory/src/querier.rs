use cosmwasm_std::{Addr, Binary, Deps, QueryRequest, StdResult, WasmQuery};
use tfi::asset::PairInfo;

pub fn query_liquidity_token(deps: Deps, contract_addr: Addr) -> StdResult<Addr> {
    // load pair_info form the pair contract
    let pair_info: PairInfo = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: contract_addr.to_string(),
        key: Binary::from("pair_info".as_bytes()),
    }))?;

    Ok(pair_info.liquidity_token)
}
