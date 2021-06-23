use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, Decimal, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, WasmMsg,
};

use crate::state::{Config, CONFIG};

use cw20::Cw20ExecuteMsg;
use tfi::asset::{Asset, AssetInfo, PairInfo};
use tfi::pair::ExecuteMsg as PairExecuteMsg;
use tfi::querier::{query_balance, query_pair_info, query_token_balance};
use tfi::router::SwapOperation;

/// Execute swap operation
/// swap all offer asset to ask asset
pub fn execute_swap_operation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operation: SwapOperation,
    to: Option<String>,
) -> StdResult<Response> {
    if env.contract.address != info.sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    let SwapOperation {
        offer_asset_info,
        ask_asset_info,
    } = operation;
    let config: Config = CONFIG.load(deps.as_ref().storage)?;
    let tfi_factory = deps.api.addr_humanize(&config.tfi_factory)?;
    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        tfi_factory,
        &[offer_asset_info.clone(), ask_asset_info],
    )?;

    let amount = match offer_asset_info.clone() {
        AssetInfo::Native(denom) => query_balance(&deps.querier, env.contract.address, denom)?,
        AssetInfo::Token(contract_addr) => {
            query_token_balance(&deps.querier, contract_addr, env.contract.address)?
        }
    };
    let offer_asset: Asset = Asset {
        info: offer_asset_info,
        amount,
    };

    let messages: Vec<CosmosMsg> = vec![asset_into_swap_msg(
        pair_info.contract_addr,
        offer_asset,
        None,
        to,
    )?];

    Ok(Response {
        messages,
        submessages: vec![],
        attributes: vec![],
        data: None,
    })
}

pub fn asset_into_swap_msg(
    pair_contract: Addr,
    offer_asset: Asset,
    max_spread: Option<Decimal>,
    to: Option<String>,
) -> StdResult<CosmosMsg> {
    match offer_asset.info.clone() {
        AssetInfo::Native(denom) => {
            let amount = offer_asset.amount;

            Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_contract.to_string(),
                send: vec![Coin { denom, amount }],
                msg: to_binary(&PairExecuteMsg::Swap {
                    offer_asset: Asset {
                        amount,
                        ..offer_asset
                    },
                    belief_price: None,
                    max_spread,
                    to,
                })?,
            }))
        }
        AssetInfo::Token(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            send: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: pair_contract.to_string(),
                amount: offer_asset.amount,
                msg: Some(to_binary(&PairExecuteMsg::Swap {
                    offer_asset,
                    belief_price: None,
                    max_spread,
                    to,
                })?),
            })?,
        })),
    }
}
