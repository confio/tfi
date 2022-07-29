use cosmwasm_std::{
    Addr, Binary, ContractInfoResponse, Deps, Env, QueryRequest, StdResult, WasmQuery,
};
use tfi::asset::PairInfo;

pub fn query_liquidity_token(deps: Deps, contract_addr: Addr) -> StdResult<Addr> {
    // load pair_info form the pair contract
    let pair_info: PairInfo = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
        contract_addr: contract_addr.to_string(),
        key: Binary::from("pair_info".as_bytes()),
    }))?;

    Ok(pair_info.liquidity_token)
}

pub fn query_migrate_admin(deps: Deps, env: &Env) -> StdResult<Option<String>> {
    let contract_info_query = QueryRequest::Wasm(WasmQuery::ContractInfo {
        contract_addr: env.contract.address.to_string(),
    });
    let contract_info = deps
        .querier
        .query::<ContractInfoResponse>(&contract_info_query)?;
    Ok(contract_info.admin)
}
