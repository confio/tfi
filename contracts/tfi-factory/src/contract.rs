#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, ContractInfoResponse, Decimal, Deps, DepsMut, Empty, Env, MessageInfo,
    QueryRequest, Reply, Response, StdError, StdResult, SubMsg, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::querier::query_liquidity_token;
use crate::response::MsgInstantiateContractResponse;
use crate::state::{pair_key, read_pairs, Config, TmpPairInfo, CONFIG, PAIRS, TMP_PAIR_INFO};

use protobuf::Message;
use tfi::asset::{AssetInfo, PairInfo};
use tfi::factory::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, PairsResponse, QueryMsg,
};
use tfi::pair::InstantiateMsg as PairInstantiateMsg;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:tfi-factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    if !(Decimal::zero()..=Decimal::one()).contains(&msg.default_commission) {
        return Err(ContractError::InvalidCommission(msg.default_commission));
    }

    let config = Config {
        owner: info.sender,
        token_code_id: msg.token_code_id,
        pair_code_id: msg.pair_code_id,
        default_commission: msg.default_commission,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            token_code_id,
            pair_code_id,
            default_commission,
        } => execute_update_config(
            deps,
            env,
            info,
            owner,
            token_code_id,
            pair_code_id,
            default_commission,
        )
        .map_err(Into::into),
        ExecuteMsg::CreatePair {
            asset_infos,
            commission,
        } => execute_create_pair(deps, env, info, asset_infos, commission),
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
    default_commission: Option<Decimal>,
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

    if let Some(commission) = default_commission {
        config.default_commission = commission;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

// Anyone can execute it to create swap pair
pub fn execute_create_pair(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    asset_infos: [AssetInfo; 2],
    commission: Option<Decimal>,
) -> Result<Response, ContractError> {
    if let Some(commission) = commission {
        if !(Decimal::zero()..=Decimal::one()).contains(&commission) {
            return Err(ContractError::InvalidCommission(commission));
        }
    }

    let config: Config = CONFIG.load(deps.storage)?;

    let pair_key = pair_key(&asset_infos);
    if let Ok(Some(_)) = PAIRS.may_load(deps.storage, &pair_key) {
        return Err(StdError::generic_err("Pair already exists").into());
    }

    let commission = commission.unwrap_or(config.default_commission);

    TMP_PAIR_INFO.save(
        deps.storage,
        &TmpPairInfo {
            pair_key,
            asset_infos: asset_infos.clone(),
            commission,
        },
    )?;

    let query = QueryRequest::<Empty>::Wasm(WasmQuery::ContractInfo {
        contract_addr: env.contract.address.to_string(),
    });
    let info = deps.querier.query::<ContractInfoResponse>(&query)?;

    let pair_name = format!("{}-{}", asset_infos[0], asset_infos[1]);
    let msg = WasmMsg::Instantiate {
        code_id: config.pair_code_id,
        funds: vec![],
        admin: info.admin,
        label: "Tgrade finance trading pair".to_string(),
        msg: to_binary(
            &PairInstantiateMsg::new(asset_infos, config.token_code_id).with_commission(commission),
        )?,
    };
    let msg = SubMsg::reply_on_success(msg, 1);
    let res = Response::new()
        .add_submessage(msg)
        .add_attribute("action", "create_pair")
        .add_attribute("pair", pair_name);
    Ok(res)
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
        &PairInfo::new(
            tmp_pair_info.asset_infos,
            pair_contract.clone(),
            liquidity_token.clone(),
        )
        .with_commission(tmp_pair_info.commission),
    )?;

    Ok(Response::new()
        .add_attribute("pair_contract_addr", pair_contract)
        .add_attribute("liquidity_token_addr", liquidity_token))
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
        default_commission: state.default_commission,
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
