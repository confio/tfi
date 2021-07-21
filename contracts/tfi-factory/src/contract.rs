#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, WasmMsg,
};

use crate::querier::query_liquidity_token;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{pair_key, read_pairs, Config, TmpPairInfo, CONFIG, PAIRS, TMP_PAIR_INFO};

use protobuf::Message;
use tfi::asset::{AssetInfo, PairInfo};
use tfi::factory::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, PairsResponse, QueryMsg,
};
use tfi::pair::InstantiateMsg as PairInstantiateMsg;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: info.sender,
        token_code_id: msg.token_code_id,
        pair_code_id: msg.pair_code_id,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![],
        submessages: vec![],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            token_code_id,
            pair_code_id,
        } => execute_update_config(deps, env, info, owner, token_code_id, pair_code_id),
        ExecuteMsg::CreatePair { asset_infos } => execute_create_pair(deps, env, info, asset_infos),
    }
}

// Only owner can execute it
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    token_code_id: Option<u64>,
    pair_code_id: Option<u64>,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        // validate address format
        let owner = deps.api.addr_validate(&owner)?;
        config.owner = owner;
    }

    if let Some(token_code_id) = token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(pair_code_id) = pair_code_id {
        config.pair_code_id = pair_code_id;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![attr("action", "update_config")],
        data: None,
        submessages: vec![],
    })
}

// Anyone can execute it to create swap pair
pub fn execute_create_pair(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    asset_infos: [AssetInfo; 2],
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    let pair_key = pair_key(&asset_infos);
    if let Ok(Some(_)) = PAIRS.may_load(deps.storage, &pair_key) {
        return Err(StdError::generic_err("Pair already exists"));
    }

    TMP_PAIR_INFO.save(
        deps.storage,
        &TmpPairInfo {
            pair_key,
            asset_infos: asset_infos.clone(),
        },
    )?;

    let pair_name = format!("{}-{}", asset_infos[0], asset_infos[1]);
    let sub_msg = SubMsg::<Empty> {
        id: 1,
        gas_limit: None,
        msg: WasmMsg::Instantiate {
            code_id: config.pair_code_id,
            send: vec![],
            admin: None,
            label: "Living the dream".to_string(),
            msg: to_binary(&PairInstantiateMsg {
                asset_infos,
                token_code_id: config.token_code_id,
            })?,
        }
        .into(),
        reply_on: ReplyOn::Success,
    };
    Ok(Response {
        messages: vec![],
        attributes: vec![attr("action", "create_pair"), attr("pair", pair_name)],
        data: None,
        submessages: vec![sub_msg],
    })
}

/// This just stores the result for future query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    let tmp_pair_info = TMP_PAIR_INFO.load(deps.storage)?;

    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(msg.result.unwrap().data.unwrap().as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    let pair_contract = deps.api.addr_validate(res.get_contract_address())?;
    let liquidity_token = query_liquidity_token(deps.as_ref(), pair_contract.clone())?;

    PAIRS.save(
        deps.storage,
        &tmp_pair_info.pair_key,
        &PairInfo {
            liquidity_token: liquidity_token.clone(),
            contract_addr: pair_contract.clone(),
            asset_infos: tmp_pair_info.asset_infos,
        },
    )?;

    Ok(Response {
        messages: vec![],
        submessages: vec![],
        attributes: vec![
            attr("pair_contract_addr", pair_contract.as_str()),
            attr("liquidity_token_addr", liquidity_token.as_str()),
        ],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Pair { asset_infos } => to_binary(&query_pair(deps, asset_infos)?),
        QueryMsg::Pairs { start_after, limit } => {
            to_binary(&query_pairs(deps, start_after, limit)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state: Config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: state.owner.into(),
        token_code_id: state.token_code_id,
        pair_code_id: state.pair_code_id,
    };

    Ok(resp)
}

pub fn query_pair(deps: Deps, asset_infos: [AssetInfo; 2]) -> StdResult<PairInfo> {
    let pair_key = pair_key(&asset_infos);
    let pair_info: PairInfo = PAIRS.load(deps.storage, &pair_key)?;
    Ok(pair_info)
}

pub fn query_pairs(
    deps: Deps,
    start_after: Option<[AssetInfo; 2]>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let pairs: Vec<PairInfo> = read_pairs(deps.storage, start_after, limit)?;
    let resp = PairsResponse { pairs };

    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
