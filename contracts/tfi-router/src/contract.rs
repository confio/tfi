use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Api, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};

use crate::operations::execute_swap_operation;
use crate::state::{Config, CONFIG};

use cw20::Cw20ReceiveMsg;
use std::collections::HashMap;
use tfi::asset::{Asset, AssetInfo, PairInfo};
use tfi::pair::{QueryMsg as PairQueryMsg, SimulationResponse};
use tfi::querier::query_pair_info;
use tfi::router::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg,
    SimulateSwapOperationsResponse, SwapOperation,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(
        deps.storage,
        &Config {
            tfi_factory: deps.api.addr_canonicalize(&msg.tfi_factory)?,
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::ExecuteSwapOperations {
            operations,
            minimum_receive,
            to,
        } => {
            let api = deps.api;
            execute_swap_operations(
                deps,
                env,
                info.sender,
                operations,
                minimum_receive,
                optional_addr_validate(api, to)?,
            )
        }
        ExecuteMsg::ExecuteSwapOperation { operation, to } => {
            let api = deps.api;
            execute_swap_operation(
                deps,
                env,
                info,
                operation,
                optional_addr_validate(api, to)?.map(|v| v.to_string()),
            )
        }
        ExecuteMsg::AssertMinimumReceive {
            asset_info,
            prev_balance,
            minimum_receive,
            receiver,
        } => assert_minium_receive(
            deps.as_ref(),
            asset_info,
            prev_balance,
            minimum_receive,
            deps.api.addr_validate(&receiver)?,
        ),
    }
}

fn optional_addr_validate(api: &dyn Api, addr: Option<String>) -> StdResult<Option<Addr>> {
    let addr = if let Some(addr) = addr {
        Some(api.addr_validate(&addr)?)
    } else {
        None
    };

    Ok(addr)
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let sender = deps.api.addr_validate(&cw20_msg.sender)?;
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::ExecuteSwapOperations {
            operations,
            minimum_receive,
            to,
        } => {
            let api = deps.api;
            execute_swap_operations(
                deps,
                env,
                sender,
                operations,
                minimum_receive,
                optional_addr_validate(api, to)?,
            )
        }
    }
}

pub fn execute_swap_operations(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    operations: Vec<SwapOperation>,
    minimum_receive: Option<Uint128>,
    to: Option<Addr>,
) -> StdResult<Response> {
    let operations_len = operations.len();
    if operations_len == 0 {
        return Err(StdError::generic_err("must provide operations"));
    }

    // Assert the operations are properly set
    assert_operations(&operations)?;

    let to = if let Some(to) = to { to } else { sender };
    let target_asset_info = operations.last().unwrap().get_target_asset_info();

    let mut operation_index = 0;
    let mut messages: Vec<CosmosMsg> = operations
        .into_iter()
        .map(|op| {
            operation_index += 1;
            Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                send: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: op,
                    to: if operation_index == operations_len {
                        Some(to.to_string())
                    } else {
                        None
                    },
                })?,
            }))
        })
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    // Execute minimum amount assertion
    if let Some(minimum_receive) = minimum_receive {
        let receiver_balance = target_asset_info.query_pool(&deps.querier, to.clone())?;
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            send: vec![],
            msg: to_binary(&ExecuteMsg::AssertMinimumReceive {
                asset_info: target_asset_info,
                prev_balance: receiver_balance,
                minimum_receive,
                receiver: to.to_string(),
            })?,
        }))
    }

    Ok(Response {
        messages,
        submessages: vec![],
        attributes: vec![],
        data: None,
    })
}

fn assert_minium_receive(
    deps: Deps,
    asset_info: AssetInfo,
    prev_balance: Uint128,
    minium_receive: Uint128,
    receiver: Addr,
) -> StdResult<Response> {
    let receiver_balance = asset_info.query_pool(&deps.querier, receiver)?;
    let swap_amount = receiver_balance.checked_sub(prev_balance)?;

    if swap_amount < minium_receive {
        return Err(StdError::generic_err(format!(
            "assertion failed; minimum receive amount: {}, swap amount: {}",
            minium_receive, swap_amount
        )));
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::SimulateSwapOperations {
            offer_amount,
            operations,
        } => to_binary(&simulate_swap_operations(deps, offer_amount, operations)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        tfi_factory: deps.api.addr_humanize(&state.tfi_factory)?.to_string(),
    };

    Ok(resp)
}

fn simulate_swap_operations(
    deps: Deps,
    offer_amount: Uint128,
    operations: Vec<SwapOperation>,
) -> StdResult<SimulateSwapOperationsResponse> {
    let config: Config = CONFIG.load(deps.storage)?;
    let tfi_factory = deps.api.addr_humanize(&config.tfi_factory)?;

    let operations_len = operations.len();
    if operations_len == 0 {
        return Err(StdError::generic_err("must provide operations"));
    }

    let mut offer_amount = offer_amount;
    for operation in operations.into_iter() {
        let SwapOperation {
            offer_asset_info,
            ask_asset_info,
        } = operation;
        let pair_info: PairInfo = query_pair_info(
            &deps.querier,
            tfi_factory.clone(),
            &[offer_asset_info.clone(), ask_asset_info.clone()],
        )?;

        let res: SimulationResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: pair_info.contract_addr.to_string(),
                msg: to_binary(&PairQueryMsg::Simulation {
                    offer_asset: Asset {
                        info: offer_asset_info,
                        amount: offer_amount,
                    },
                })?,
            }))?;

        offer_amount = res.return_amount;
    }

    Ok(SimulateSwapOperationsResponse {
        amount: offer_amount,
    })
}

fn assert_operations(operations: &[SwapOperation]) -> StdResult<()> {
    let mut ask_asset_map: HashMap<String, bool> = HashMap::new();
    for operation in operations.iter() {
        ask_asset_map.remove(&operation.offer_asset_info.to_string());
        ask_asset_map.insert(operation.ask_asset_info.to_string(), true);
    }

    if ask_asset_map.keys().len() != 1 {
        return Err(StdError::generic_err(
            "invalid operations; multiple output token",
        ));
    }

    Ok(())
}

#[test]
fn test_invalid_operations() {
    // empty error
    assert_eq!(true, assert_operations(&[]).is_err());

    // uluna output
    assert_eq!(
        true,
        assert_operations(&[
            SwapOperation {
                offer_asset_info: AssetInfo::Native("ukrw".to_string()),
                ask_asset_info: AssetInfo::Token(Addr::unchecked("asset0001")),
            },
            SwapOperation {
                offer_asset_info: AssetInfo::Token(Addr::unchecked("asset0001")),
                ask_asset_info: AssetInfo::Native("uluna".to_string()),
            }
        ])
        .is_ok()
    );

    // asset0002 output
    assert_eq!(
        true,
        assert_operations(&[
            SwapOperation {
                offer_asset_info: AssetInfo::Native("ukrw".to_string()),
                ask_asset_info: AssetInfo::Token(Addr::unchecked("asset0001")),
            },
            SwapOperation {
                offer_asset_info: AssetInfo::Token(Addr::unchecked("asset0001")),
                ask_asset_info: AssetInfo::Native("uluna".to_string()),
            },
            SwapOperation {
                offer_asset_info: AssetInfo::Native("uluna".to_string()),
                ask_asset_info: AssetInfo::Token(Addr::unchecked("asset0002")),
            },
        ])
        .is_ok()
    );

    // multiple output token types error
    assert_eq!(
        true,
        assert_operations(&[
            SwapOperation {
                offer_asset_info: AssetInfo::Native("ukrw".to_string()),
                ask_asset_info: AssetInfo::Token(Addr::unchecked("asset0001")),
            },
            SwapOperation {
                offer_asset_info: AssetInfo::Token(Addr::unchecked("asset0001")),
                ask_asset_info: AssetInfo::Native("uaud".to_string()),
            },
            SwapOperation {
                offer_asset_info: AssetInfo::Native("uluna".to_string()),
                ask_asset_info: AssetInfo::Token(Addr::unchecked("asset0002")),
            },
        ])
        .is_err()
    );
}
