use crate::error::ContractError;
use crate::math::{decimal_multiplication, decimal_subtraction, reverse_decimal};
use crate::state::PAIR_INFO;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use integer_sqrt::IntegerSquareRoot;
use std::str::FromStr;
use tfi::asset::{Asset, AssetInfo, PairInfo};
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
    let pair_info: &PairInfo = &PairInfo {
        contract_addr: env.contract.address.clone(),
        // ugly placeholder, but we set this in the callback
        liquidity_token: Addr::unchecked(""),
        asset_infos: msg.asset_infos,
    };

    PAIR_INFO.save(deps.storage, &pair_info)?;

    let token_init = &TokenInstantiateMsg {
        name: "tfi liquidity token".to_string(),
        symbol: "uLP".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: env.contract.address.to_string(),
            cap: None,
        }),
    };
    let msg = WasmMsg::Instantiate {
        admin: None,
        code_id: msg.token_code_id,
        msg: to_binary(&token_init)?,
        funds: vec![],
        label: "uLP liquidity token".to_string(),
    };
    let msg = SubMsg::reply_on_success(msg, 1);
    Ok(Response::new().add_submessage(msg))
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
            let config: PairInfo = PAIR_INFO.load(deps.storage)?;
            let pools: [Asset; 2] =
                config.query_pools(&deps.querier, env.contract.address.clone())?;
            for pool in pools.iter() {
                if let AssetInfo::Token(contract_addr) = &pool.info {
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

            let api = deps.api;
            swap(
                deps,
                env,
                info,
                api.addr_validate(&cw20_msg.sender)?,
                Asset {
                    info: AssetInfo::Token(contract_addr),
                    amount: cw20_msg.amount,
                },
                belief_price,
                max_spread,
                to_addr,
            )
        }
        Ok(Cw20HookMsg::WithdrawLiquidity {}) => {
            let config: PairInfo = PAIR_INFO.load(deps.storage)?;
            if info.sender != config.liquidity_token {
                return Err(ContractError::Unauthorized {});
            }

            let sender_addr = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            withdraw_liquidity(deps, env, info, sender_addr, cw20_msg.amount)
        }
        Err(err) => Err(ContractError::Std(err)),
    }
}

// This parses the contract_addr returned from init data
// message MsgInstantiateContractResponse {
//   string contract_address = 1;
//   bytes data = 2;
// }
// Let's do this by hand to avoid whole protobuf libs
fn parse_init_addr(init_result: &[u8]) -> Result<&str, ContractError> {
    if init_result.len() < 2 {
        return Err(ContractError::InvalidAddressLength(init_result.len()));
    }

    // ensure the first byte (field 1, type 2 = 1 << 3 + 2 = 10)
    if init_result[0] != 10 {
        return Err(StdError::generic_err("Unexpected field, must be 10").into());
    }
    // parse the length (this will always be less than 127 in our case)
    let length = init_result[1] as usize;

    if init_result.len() < 2 + length {
        return Err(ContractError::InvalidAddressLength(init_result.len()));
    }

    let addr_bytes = &init_result[2..][..length];

    Ok(std::str::from_utf8(addr_bytes).map_err(StdError::from)?)
}

/// This just stores the result for future query
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    // this is the only expected one from init
    if msg.id != 1 {
        return Err(StdError::generic_err("Unsupported reply id").into());
    }

    let data = msg
        .result
        .into_result()
        .map_err(ContractError::MessageFailure)?
        .data
        .ok_or(ContractError::MissingData {})?;
    let contract_addr = parse_init_addr(&data)?;
    let liquidity_token = deps.api.addr_validate(contract_addr)?;

    PAIR_INFO.update(deps.storage, |mut meta| -> StdResult<_> {
        meta.liquidity_token = liquidity_token;
        Ok(meta)
    })?;

    Ok(Response::new().add_attribute("liquidity_token_addr", contract_addr))
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

    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;
    // we really should do this locally...
    let mut pools: [Asset; 2] =
        pair_info.query_pools(&deps.querier, env.contract.address.clone())?;
    let deposits: [Uint128; 2] = [
        assets
            .iter()
            .find(|a| a.info.equal(&pools[0].info))
            .ok_or_else(|| ContractError::AssetMismatch(pools[0].info.to_string()))?
            .amount,
        assets
            .iter()
            .find(|a| a.info.equal(&pools[1].info))
            .ok_or_else(|| ContractError::AssetMismatch(pools[1].info.to_string()))?
            .amount,
    ];

    let mut res = Response::new()
        .add_attribute("action", "provide_liquidity")
        .add_attribute("assets", format!("{}, {}", assets[0], assets[1]));

    for (i, pool) in pools.iter_mut().enumerate() {
        // If the pool is token contract, then we need to execute TransferFrom msg to receive funds
        if let AssetInfo::Token(contract_addr) = &pool.info {
            res = res.add_message(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount: deposits[i],
                })?,
                funds: vec![],
            });
        } else {
            // If the asset is native token, balance is already increased
            // To calculated properly we should subtract user deposit from the pool
            pool.amount = pool.amount.checked_sub(deposits[i])?;
        }
    }

    // assert slippage tolerance
    assert_slippage_tolerance(&slippage_tolerance, &deposits, &pools)?;

    let total_share = query_supply(&deps.querier, pair_info.liquidity_token.clone())?;
    let share = if total_share == Uint128::zero() {
        // Initial share = collateral amount
        Uint128::new((deposits[0].u128() * deposits[1].u128()).integer_sqrt())
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

    Ok(res.
        add_attribute("share", share.to_string()).
        // mint LP token to sender
        add_message(WasmMsg::Execute {
        contract_addr: pair_info.liquidity_token.into(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.to_string(),
            amount: share,
        })?,
        funds: vec![],
    }))
}

pub fn withdraw_liquidity(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;

    let pools: [Asset; 2] = pair_info.query_pools(&deps.querier, env.contract.address)?;
    let total_share: Uint128 = query_supply(&deps.querier, pair_info.liquidity_token.clone())?;

    let share_ratio: Decimal = Decimal::from_ratio(amount, total_share);
    let refund_assets: Vec<Asset> = pools
        .iter()
        .map(|a| Asset {
            info: a.info.clone(),
            amount: a.amount * share_ratio,
        })
        .collect();

    // update pool info
    let res = Response::new()
        // refund asset tokens
        .add_message(refund_assets[0].clone().into_msg(sender.clone())?)
        .add_message(refund_assets[1].clone().into_msg(sender)?)
        // burn liquidity token
        .add_message(WasmMsg::Execute {
            contract_addr: pair_info.liquidity_token.into(),
            msg: to_binary(&Cw20ExecuteMsg::Burn { amount })?,
            funds: vec![],
        })
        .add_attribute("action", "withdraw_liquidity")
        .add_attribute("withdrawn_share", amount.to_string())
        .add_attribute(
            "refund_assets",
            format!("{}, {}", refund_assets[0], refund_assets[1]),
        );
    Ok(res)
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

    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;

    let pools: [Asset; 2] = pair_info.query_pools(&deps.querier, env.contract.address)?;

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
        return Err(ContractError::AssetMismatch(offer_asset.info.to_string()));
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

    let return_msg = Asset {
        info: ask_pool.info.clone(),
        amount: return_amount,
    }
    .into_msg(to.unwrap_or(sender))?;

    // 1. send collateral token from the contract to a user
    // 2. send inactive commission to collector
    let res = Response::new()
        .add_attribute("action", "swap")
        .add_attribute("offer_asset", offer_asset.info.to_string())
        .add_attribute("ask_asset", ask_pool.info.to_string())
        .add_attribute("offer_amount", offer_amount.to_string())
        .add_attribute("return_amount", return_amount.to_string())
        .add_attribute("spread_amount", spread_amount.to_string())
        .add_attribute("commission_amount", commission_amount.to_string())
        .add_message(return_msg);
    Ok(res)
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
    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;
    Ok(pair_info)
}

pub fn query_pool(deps: Deps) -> Result<PoolResponse, ContractError> {
    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;
    let assets: [Asset; 2] =
        pair_info.query_pools(&deps.querier, pair_info.contract_addr.clone())?;
    let total_share: Uint128 = query_supply(&deps.querier, pair_info.liquidity_token)?;

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
    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;

    let pools: [Asset; 2] =
        pair_info.query_pools(&deps.querier, pair_info.contract_addr.clone())?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if offer_asset.info.equal(&pools[0].info) {
        offer_pool = pools[0].clone();
        ask_pool = pools[1].clone();
    } else if offer_asset.info.equal(&pools[1].info) {
        offer_pool = pools[1].clone();
        ask_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch(offer_asset.info.to_string()));
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
    let pair_info: PairInfo = PAIR_INFO.load(deps.storage)?;

    let pools: [Asset; 2] =
        pair_info.query_pools(&deps.querier, pair_info.contract_addr.clone())?;

    let offer_pool: Asset;
    let ask_pool: Asset;
    if ask_asset.info.equal(&pools[0].info) {
        ask_pool = pools[0].clone();
        offer_pool = pools[1].clone();
    } else if ask_asset.info.equal(&pools[1].info) {
        ask_pool = pools[1].clone();
        offer_pool = pools[0].clone();
    } else {
        return Err(ContractError::AssetMismatch(ask_asset.info.to_string()));
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
    let cp = Uint128::new(offer_pool.u128() * ask_pool.u128());
    let return_amount =
        ask_pool.checked_sub(cp.multiply_ratio(1u128, offer_pool + offer_amount))?;

    // calculate spread & commission
    if offer_pool.is_zero() {
        // return Err(StdError::divide_by_zero(ask_pool.to_string()).into());
        return Err(StdError::generic_err(
            "Divide by zero error computing the swap",
        ));
    }
    let spread_amount: Uint128 = (offer_amount * Decimal::from_ratio(ask_pool, offer_pool))
        .checked_sub(return_amount)
        .unwrap_or_else(|_| Uint128::zero());
    let commission_amount: Uint128 = return_amount * Decimal::from_str(&COMMISSION_RATE).unwrap();

    // commission will be absorbed to pool
    let return_amount: Uint128 = return_amount.checked_sub(commission_amount)?;

    Ok((return_amount, spread_amount, commission_amount))
}

fn compute_offer_amount(
    offer_pool: Uint128,
    ask_pool: Uint128,
    ask_amount: Uint128,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    // ask => offer
    // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
    let cp = Uint128::new(offer_pool.u128() * ask_pool.u128());
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
