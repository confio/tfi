use crate::error::ContractError;
use crate::math::{decimal_multiplication, decimal_subtraction, reverse_decimal};
use crate::response::MsgInstantiateContractResponse;
use crate::state::PAIR_INFO;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, CanonicalAddr, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Uint128,
    WasmMsg,
};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use integer_sqrt::IntegerSquareRoot;
use protobuf::Message;
use std::str::FromStr;
use tfi::asset::{Asset, AssetInfo, PairInfo, PairInfoRaw};
use tfi::pair::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, PoolResponse, QueryMsg,
    ReverseSimulationResponse, SimulationResponse,
};
use tfi::querier::query_supply;
use tfi::token::InstantiateMsg as TokenInstantiateMsg;

/// Commission rate == 0.3%
const COMMISSION_RATE: &str = "0.003";
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let pair_info: &PairInfoRaw = &PairInfoRaw {
        contract_addr: deps.api.addr_canonicalize(env.contract.address.as_str())?,
        liquidity_token: CanonicalAddr::from(vec![]),
        asset_infos: [
            msg.asset_infos[0].to_raw(deps.api)?,
            msg.asset_infos[1].to_raw(deps.api)?,
        ],
    };

    PAIR_INFO.save(deps.storage, &pair_info)?;

    Ok(Response {
        messages: vec![],
        submessages: vec![SubMsg {
            // Create LP token
            msg: WasmMsg::Instantiate {
                admin: None,
                code_id: msg.token_code_id,
                msg: to_binary(&TokenInstantiateMsg {
                    name: "tfi liquidity token".to_string(),
                    symbol: "uLP".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: env.contract.address.to_string(),
                        cap: None,
                    }),
                })?,
                send: vec![],
                label: "".to_string(),
            }
            .into(),
            gas_limit: None,
            id: 1,
            reply_on: ReplyOn::Success,
        }],
        attributes: vec![],
        data: None,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::ProvideLiquidity {
            assets,
            slippage_tolerance,
        } => provide_liquidity(deps, env, info, assets, slippage_tolerance),
        ExecuteMsg::Swap {
            offer_asset,
            belief_price,
            max_spread,
            to,
        } => {
            if !offer_asset.is_native_token() {
                return Err(ContractError::Unauthorized {});
            }

            let to_addr = if let Some(to_addr) = to {
                Some(deps.api.addr_validate(&to_addr)?)
            } else {
                None
            };

            swap(
                deps,
                env,
                info.clone(),
                info.sender,
                offer_asset,
                belief_price,
                max_spread,
                to_addr,
            )
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let contract_addr = info.sender.clone();

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Swap {
            belief_price,
            max_spread,
            to,
        }) => {
            // only asset contract can execute this message
            let mut authorized: bool = false;
            let config: PairInfoRaw = PAIR_INFO.load(deps.storage)?;
            let pools: [Asset; 2] =
                config.query_pools(&deps.querier, deps.api, env.contract.address.clone())?;
            for pool in pools.iter() {
                if let AssetInfo::Token { contract_addr, .. } = &pool.info {
                    if contract_addr == &info.sender {
                        authorized = true;
                    }
                }
            }

            if !authorized {
                return Err(ContractError::Unauthorized {});
            }

            let to_addr = if let Some(to_addr) = to {
                Some(deps.api.addr_validate(to_addr.as_str())?)
            } else {
                None
            };

            swap(
                deps,
                env,
                info,
                Addr::unchecked(cw20_msg.sender),
                Asset {
                    info: AssetInfo::Token { contract_addr },
                    amount: cw20_msg.amount,
                },
                belief_price,
                max_spread,
                to_addr,
            )
        }
        Ok(Cw20HookMsg::WithdrawLiquidity {}) => {
            let config: PairInfoRaw = PAIR_INFO.load(deps.storage)?;
            if deps.api.addr_canonicalize(info.sender.as_str())? != config.liquidity_token {
                return Err(ContractError::Unauthorized {});
            }

            let sender_addr = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            withdraw_liquidity(deps, env, info, sender_addr, cw20_msg.amount)
        }
        Err(err) => Err(ContractError::Std(err)),
    }
}

/// This just stores the result for future query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    let data = msg.result.unwrap().data.unwrap();
    let res: MsgInstantiateContractResponse =
        Message::parse_from_bytes(data.as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;
    let liquidity_token = res.get_contract_address();

    let api = deps.api;
    PAIR_INFO.update(deps.storage, |mut meta| -> StdResult<_> {
        meta.liquidity_token = api.addr_canonicalize(liquidity_token)?;
        Ok(meta)
    })?;

    Ok(Response {
        messages: vec![],
        submessages: vec![],
        attributes: vec![attr("liquidity_token_addr", liquidity_token)],
        data: None,
    })
}

/// CONTRACT - should approve contract to use the amount of token
pub fn provide_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: [Asset; 2],
    slippage_tolerance: Option<Decimal>,
) -> Result<Response, ContractError> {
    for asset in assets.iter() {
        asset.assert_sent_native_token_balance(&info)?;
    }

    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;
    let mut pools: [Asset; 2] =
        pair_info.query_pools(&deps.querier, deps.api, env.contract.address.clone())?;
    let deposits: [Uint128; 2] = [
        assets
            .iter()
            .find(|a| a.info.equal(&pools[0].info))
            .map(|a| a.amount)
            .expect("Wrong asset info is given"),
        assets
            .iter()
            .find(|a| a.info.equal(&pools[1].info))
            .map(|a| a.amount)
            .expect("Wrong asset info is given"),
    ];

    let mut messages: Vec<CosmosMsg> = vec![];
    for (i, pool) in pools.iter_mut().enumerate() {
        // If the pool is token contract, then we need to execute TransferFrom msg to receive funds
        if let AssetInfo::Token { contract_addr, .. } = &pool.info {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: deposits[i],
                })?,
                send: vec![],
            }));
        } else {
            // If the asset is native token, balance is already increased
            // To calculated properly we should subtract user deposit from the pool
            pool.amount = pool.amount.checked_sub(deposits[i])?;
        }
    }

    // assert slippage tolerance
    assert_slippage_tolerance(&slippage_tolerance, &deposits, &pools)?;

    let liquidity_token = deps.api.addr_humanize(&pair_info.liquidity_token)?;
    let total_share = query_supply(&deps.querier, liquidity_token)?;
    let share = if total_share == Uint128::zero() {
        // Initial share = collateral amount
        Uint128((deposits[0].u128() * deposits[1].u128()).integer_sqrt())
    } else {
        // min(1, 2)
        // 1. sqrt(deposit_0 * exchange_rate_0_to_1 * deposit_0) * (total_share / sqrt(pool_0 * pool_1))
        // == deposit_0 * total_share / pool_0
        // 2. sqrt(deposit_1 * exchange_rate_1_to_0 * deposit_1) * (total_share / sqrt(pool_1 * pool_1))
        // == deposit_1 * total_share / pool_1
        std::cmp::min(
            deposits[0].multiply_ratio(total_share, pools[0].amount),
            deposits[1].multiply_ratio(total_share, pools[1].amount),
        )
    };

    // mint LP token to sender
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps
            .api
            .addr_humanize(&pair_info.liquidity_token)?
            .to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.to_string(),
            amount: share,
        })?,
        send: vec![],
    }));
    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "provide_liquidity"),
            attr("assets", format!("{}, {}", assets[0], assets[1])),
            attr("share", &share),
        ],
        data: None,
        submessages: vec![],
    })
}

pub fn withdraw_liquidity(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;
    let liquidity_addr: Addr = deps.api.addr_humanize(&pair_info.liquidity_token)?;

    let pools: [Asset; 2] = pair_info.query_pools(&deps.querier, deps.api, env.contract.address)?;
    let total_share: Uint128 = query_supply(&deps.querier, liquidity_addr)?;

    let share_ratio: Decimal = Decimal::from_ratio(amount, total_share);
    let refund_assets: Vec<Asset> = pools
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: a.amount * share_ratio,
        })
        .collect();

    // update pool info
    Ok(Response {
        messages: vec![
            // refund asset tokens
            refund_assets[0]
                .clone()
                .into_msg(&deps.querier, sender.clone())?,
            refund_assets[1].clone().into_msg(&deps.querier, sender)?,
            // burn liquidity token
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps
                    .api
                    .addr_humanize(&pair_info.liquidity_token)?
                    .to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Burn { amount })?,
                send: vec![],
            }),
        ],
        attributes: vec![
            attr("action", "withdraw_liquidity"),
            attr("withdrawn_share", &amount.to_string()),
            attr(
                "refund_assets",
                format!("{}, {}", refund_assets[0], refund_assets[1]),
            ),
        ],
        data: None,
        submessages: vec![],
    })
}

// CONTRACT - a user must do token approval
#[allow(clippy::too_many_arguments)]
pub fn swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Addr,
    offer_asset: Asset,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    to: Option<Addr>,
) -> Result<Response, ContractError> {
    offer_asset.assert_sent_native_token_balance(&info)?;

    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;

    let pools: [Asset; 2] = pair_info.query_pools(&deps.querier, deps.api, env.contract.address)?;

    let offer_pool: Asset;
    let ask_pool: Asset;

    // If the asset balance is already increased
    // To calculated properly we should subtract user deposit from the pool
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = Asset {
            amount: pools[0].amount.checked_sub(offer_asset.amount)?,
            info: pools[0].info.clone(),
        };
        ask_pool = pools[1].clone();
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = Asset {
            amount: pools[1].amount.checked_sub(offer_asset.amount)?,
            info: pools[1].info.clone(),
        };
        ask_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let offer_amount = offer_asset.amount;
    let (return_amount, spread_amount, commission_amount) =
        compute_swap(offer_pool.amount, ask_pool.amount, offer_amount)?;

    // check max spread limit if exist
    assert_max_spread(
        belief_price,
        max_spread,
        offer_amount,
        return_amount + commission_amount,
        spread_amount,
    )?;

    // compute tax
    let return_asset = Asset {
        info: ask_pool.info.clone(),
        amount: return_amount,
    };

    let tax_amount = return_asset.compute_tax(&deps.querier)?;

    // 1. send collateral token from the contract to a user
    // 2. send inactive commission to collector
    Ok(Response {
        messages: vec![return_asset.into_msg(&deps.querier, to.unwrap_or(sender))?],
        attributes: vec![
            attr("action", "swap"),
            attr("offer_asset", offer_asset.info.to_string()),
            attr("ask_asset", ask_pool.info.to_string()),
            attr("offer_amount", offer_amount.to_string()),
            attr("return_amount", return_amount.to_string()),
            attr("tax_amount", tax_amount.to_string()),
            attr("spread_amount", spread_amount.to_string()),
            attr("commission_amount", commission_amount.to_string()),
        ],
        data: None,
        submessages: vec![],
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Pair {} => Ok(to_binary(&query_pair_info(deps)?)?),
        QueryMsg::Pool {} => Ok(to_binary(&query_pool(deps)?)?),
        QueryMsg::Simulation { offer_asset } => {
            Ok(to_binary(&query_simulation(deps, offer_asset)?)?)
        }
        QueryMsg::ReverseSimulation { ask_asset } => {
            Ok(to_binary(&query_reverse_simulation(deps, ask_asset)?)?)
        }
    }
}

pub fn query_pair_info(deps: Deps) -> Result<PairInfo, ContractError> {
    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;
    let pair_info = pair_info.to_normal(deps.api)?;

    Ok(pair_info)
}

pub fn query_pool(deps: Deps) -> Result<PoolResponse, ContractError> {
    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;
    let contract_addr = deps.api.addr_humanize(&pair_info.contract_addr)?;
    let assets: [Asset; 2] = pair_info.query_pools(&deps.querier, deps.api, contract_addr)?;
    let total_share: Uint128 = query_supply(
        &deps.querier,
        deps.api.addr_humanize(&pair_info.liquidity_token)?,
    )?;

    let resp = PoolResponse {
        assets,
        total_share,
    };

    Ok(resp)
}

pub fn query_simulation(
    deps: Deps,
    offer_asset: Asset,
) -> Result<SimulationResponse, ContractError> {
    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;

    let contract_addr = deps.api.addr_humanize(&pair_info.contract_addr)?;
    let pools: [Asset; 2] = pair_info.query_pools(&deps.querier, deps.api, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = pools[0].clone();
        ask_pool = pools[1].clone();
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = pools[1].clone();
        ask_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let (return_amount, spread_amount, commission_amount) =
        compute_swap(offer_pool.amount, ask_pool.amount, offer_asset.amount)?;

    Ok(SimulationResponse {
        return_amount,
        spread_amount,
        commission_amount,
    })
}

pub fn query_reverse_simulation(
    deps: Deps,
    ask_asset: Asset,
) -> Result<ReverseSimulationResponse, ContractError> {
    let pair_info: PairInfoRaw = PAIR_INFO.load(deps.storage)?;

    let contract_addr = deps.api.addr_humanize(&pair_info.contract_addr)?;
    let pools: [Asset; 2] = pair_info.query_pools(&deps.querier, deps.api, contract_addr)?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if ask_asset.info.equal(&pools[0].info) {
        ask_pool = pools[0].clone();
        offer_pool = pools[1].clone();
    } else if ask_asset.info.equal(&pools[1].info) {
        ask_pool = pools[1].clone();
        offer_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch {});
    }

    let (offer_amount, spread_amount, commission_amount) =
        compute_offer_amount(offer_pool.amount, ask_pool.amount, ask_asset.amount)?;

    Ok(ReverseSimulationResponse {
        offer_amount,
        spread_amount,
        commission_amount,
    })
}

pub fn amount_of(coins: &[Coin], denom: String) -> Uint128 {
    match coins.iter().find(|x| x.denom == denom) {
        Some(coin) => coin.amount,
        None => Uint128::zero(),
    }
}

fn compute_swap(
    offer_pool: Uint128,
    ask_pool: Uint128,
    offer_amount: Uint128,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // offer => ask
    // ask_amount = (ask_pool - cp / (offer_pool + offer_amount)) * (1 - commission_rate)
    let cp = Uint128(offer_pool.u128() * ask_pool.u128());
    let return_amount =
        ask_pool.checked_sub(cp.multiply_ratio(1u128, offer_pool + offer_amount))?;

    // calculate spread & commission
    let spread_amount: Uint128 = (offer_amount * Decimal::from_ratio(ask_pool, offer_pool))
        .checked_sub(return_amount)
        .unwrap_or_else(|_| Uint128::zero());
    let commission_amount: Uint128 = return_amount * Decimal::from_str(&COMMISSION_RATE).unwrap();

    // commission will be absorbed to pool
    let return_amount: Uint128 = return_amount.checked_sub(commission_amount).unwrap();

    Ok((return_amount, spread_amount, commission_amount))
}

fn compute_offer_amount(
    offer_pool: Uint128,
    ask_pool: Uint128,
    ask_amount: Uint128,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // ask => offer
    // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
    let cp = Uint128(offer_pool.u128() * ask_pool.u128());
    let one_minus_commission =
        decimal_subtraction(Decimal::one(), Decimal::from_str(&COMMISSION_RATE).unwrap())?;

    let offer_amount: Uint128 = cp
        .multiply_ratio(
            1u128,
            ask_pool.checked_sub(ask_amount * reverse_decimal(one_minus_commission))?,
        )
        .checked_sub(offer_pool)?;

    let before_commission_deduction = ask_amount * reverse_decimal(one_minus_commission);
    let spread_amount = (offer_amount * Decimal::from_ratio(ask_pool, offer_pool))
        .checked_sub(before_commission_deduction)
        .unwrap_or_else(|_| Uint128::zero());
    let commission_amount =
        before_commission_deduction * Decimal::from_str(&COMMISSION_RATE).unwrap();
    Ok((offer_amount, spread_amount, commission_amount))
}

/// If `belief_price` and `max_spread` both are given,
/// we compute new spread else we just use tfi
/// spread to check `max_spread`
pub fn assert_max_spread(
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    offer_amount: Uint128,
    return_amount: Uint128,
    spread_amount: Uint128,
) -> Result<(), ContractError> {
    if let (Some(max_spread), Some(belief_price)) = (max_spread, belief_price) {
        let expected_return = offer_amount * reverse_decimal(belief_price);
        let spread_amount = expected_return
            .checked_sub(return_amount)
            .unwrap_or_else(|_| Uint128::zero());

        if return_amount < expected_return
            && Decimal::from_ratio(spread_amount, expected_return) > max_spread
        {
            return Err(ContractError::MaxSpreadAssertion {});
        }
    } else if let Some(max_spread) = max_spread {
        if Decimal::from_ratio(spread_amount, return_amount + spread_amount) > max_spread {
            return Err(ContractError::MaxSpreadAssertion {});
        }
    }

    Ok(())
}

fn assert_slippage_tolerance(
    slippage_tolerance: &Option<Decimal>,
    deposits: &[Uint128; 2],
    pools: &[Asset; 2],
) -> Result<(), ContractError> {
    if let Some(slippage_tolerance) = *slippage_tolerance {
        let one_minus_slippage_tolerance = decimal_subtraction(Decimal::one(), slippage_tolerance)?;

        // Ensure each prices are not dropped as much as slippage tolerance rate
        if decimal_multiplication(
            Decimal::from_ratio(deposits[0], deposits[1]),
            one_minus_slippage_tolerance,
        ) > Decimal::from_ratio(pools[0].amount, pools[1].amount)
            || decimal_multiplication(
                Decimal::from_ratio(deposits[1], deposits[0]),
                one_minus_slippage_tolerance,
            ) > Decimal::from_ratio(pools[1].amount, pools[0].amount)
        {
            return Err(ContractError::MaxSlippageAssertion {});
        }
    }

    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}