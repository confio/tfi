use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, Event, MessageInfo, Order, Response,
    StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20_base::allowances::query_allowance;
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};
use cw20_base::state::{BALANCES, TOKEN_INFO};
use cw20_base::ContractError as Cw20ContractError;
use cw4::Cw4Contract;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{
    AllReedemsResponse, ExecuteMsg, InstantiateMsg, IsWhitelistedResponse, QueryMsg, ReedemInfo,
    ReedemResponse, WhitelistResponse,
};
use crate::state::{Reedem, REEDEMS, WHITELIST};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:dso-token";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[entry_point]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let cw20_msg = cw20_base::msg::InstantiateMsg {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        initial_balances: msg.initial_balances,
        mint: msg.mint,
        marketing: msg.marketing,
    };
    cw20_base::contract::instantiate(deps.branch(), env, info, cw20_msg)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let addr = deps.api.addr_validate(&msg.whitelist_group)?;
    let contract = Cw4Contract(addr);
    // verify the whitelist contract is actually cw4
    contract.list_members(&deps.querier, None, Some(1))?;
    WHITELIST.save(deps.storage, &contract)?;

    Ok(Response::default())
}

pub(crate) fn verify_sender_on_whitelist(deps: Deps, sender: &Addr) -> Result<(), ContractError> {
    let whitelist = WHITELIST.load(deps.storage)?;
    if whitelist.is_member(&deps.querier, sender)?.is_none() {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

pub(crate) fn verify_sender_and_addresses_on_whitelist(
    deps: Deps,
    sender: &Addr,
    addresses: &[&str],
) -> Result<(), ContractError> {
    let whitelist = WHITELIST.load(deps.storage)?;
    if whitelist.is_member(&deps.querier, sender)?.is_none() {
        return Err(ContractError::Unauthorized {});
    }
    for address in addresses {
        let validated_address = deps.api.addr_validate(&address)?;
        if whitelist
            .is_member(&deps.querier, &validated_address)?
            .is_none()
        {
            return Err(ContractError::Unauthorized {});
        }
    }
    Ok(())
}

/// Reedems token effectively burning them and storing information about reedem internally. This
/// also triggers custom `reedem` event with details of process. Before reedeming, sender should
/// make sure, that token provider is aware about such possibility and is willing to cover reedem
/// off-chain, otherwise this may be equivalent to destrotying commodity.
fn execute_reedem(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    code: String,
    sender: Option<String>,
    memo: String,
) -> Result<Response, ContractError> {
    if REEDEMS.has(deps.storage, code.clone()) {
        return Err(ContractError::ReedemCodeUsed {});
    }

    if amount == Uint128::zero() {
        return Err(Cw20ContractError::InvalidZeroAmount {}.into());
    }

    // lower balance
    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    // reduce total_supply
    TOKEN_INFO.update(deps.storage, |mut info| -> StdResult<_> {
        info.total_supply = info.total_supply.checked_sub(amount)?;
        Ok(info)
    })?;

    REEDEMS.save(
        deps.storage,
        code.clone(),
        &Reedem {
            sender: info.sender.clone(),
            amount,
            memo: memo.clone(),
            timestamp: env.block.time,
        },
    )?;

    let sender = if let Some(sender) = sender {
        deps.api.addr_validate(&sender)?;
        sender
    } else {
        info.sender.to_string()
    };

    let event = Event::new("reedem")
        .add_attribute("code", code)
        .add_attribute("sender", sender)
        .add_attribute("amount", amount)
        .add_attribute("memo", memo);

    Ok(Response::new()
        .add_event(event)
        .add_attribute("action", "reedem")
        .add_attribute("from", info.sender)
        .add_attribute("amount", amount))
}

/// Removes info about reedems from contract, can be performed by minter only
fn execute_remove_reedems(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    codes: Vec<String>,
) -> Result<Response, ContractError> {
    let config = TOKEN_INFO.load(deps.storage)?;
    if config.mint.is_none() || config.mint.as_ref().unwrap().minter != info.sender {
        return Err(Cw20ContractError::Unauthorized {}.into());
    }

    for code in codes {
        REEDEMS.remove(deps.storage, code);
    }

    Ok(Response::new().add_attribute("action", "remove_reedems"))
}

/// Removes all reedems info from contract
fn execute_clean_reedems(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = TOKEN_INFO.load(deps.storage)?;
    if config.mint.is_none() || config.mint.as_ref().unwrap().minter != info.sender {
        return Err(Cw20ContractError::Unauthorized {}.into());
    }

    let keys: Vec<_> = REEDEMS
        .keys(deps.storage, None, None, Order::Ascending)
        .collect();

    for key in keys {
        REEDEMS.remove(
            deps.storage,
            String::from_utf8(key).map_err(StdError::from)?,
        )
    }

    Ok(Response::new().add_attribute("action", "remove_all_reedems"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let res = match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            verify_sender_and_addresses_on_whitelist(deps.as_ref(), &info.sender, &[&recipient])?;
            cw20_base::contract::execute_transfer(deps, env, info, recipient, amount)?
        }
        ExecuteMsg::Burn { amount } => {
            verify_sender_on_whitelist(deps.as_ref(), &info.sender)?;
            cw20_base::contract::execute_burn(deps, env, info, amount)?
        }
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => {
            verify_sender_and_addresses_on_whitelist(deps.as_ref(), &info.sender, &[&contract])?;
            cw20_base::contract::execute_send(deps, env, info, contract, amount, msg)?
        }
        ExecuteMsg::Mint { recipient, amount } => {
            verify_sender_and_addresses_on_whitelist(deps.as_ref(), &info.sender, &[&recipient])?;
            cw20_base::contract::execute_mint(deps, env, info, recipient, amount)?
        }
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => {
            verify_sender_on_whitelist(deps.as_ref(), &info.sender)?;
            cw20_base::allowances::execute_increase_allowance(
                deps, env, info, spender, amount, expires,
            )?
        }
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => {
            verify_sender_on_whitelist(deps.as_ref(), &info.sender)?;
            cw20_base::allowances::execute_decrease_allowance(
                deps, env, info, spender, amount, expires,
            )?
        }
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => {
            verify_sender_and_addresses_on_whitelist(
                deps.as_ref(),
                &info.sender,
                &[&owner, &recipient],
            )?;
            cw20_base::allowances::execute_transfer_from(deps, env, info, owner, recipient, amount)?
        }
        ExecuteMsg::BurnFrom { owner, amount } => {
            verify_sender_and_addresses_on_whitelist(deps.as_ref(), &info.sender, &[&owner])?;
            cw20_base::allowances::execute_burn_from(deps, env, info, owner, amount)?
        }
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => {
            verify_sender_and_addresses_on_whitelist(
                deps.as_ref(),
                &info.sender,
                &[&owner, &contract],
            )?;
            cw20_base::allowances::execute_send_from(deps, env, info, owner, contract, amount, msg)?
        }
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => cw20_base::contract::execute_update_marketing(
            deps,
            env,
            info,
            project,
            description,
            marketing,
        )?,
        ExecuteMsg::UploadLogo(logo) => {
            cw20_base::contract::execute_upload_logo(deps, env, info, logo)?
        }
        ExecuteMsg::Reedem {
            amount,
            code,
            sender,
            memo,
        } => execute_reedem(deps, env, info, amount, code, sender, memo)?,
        ExecuteMsg::RemoveReedems { codes } => execute_remove_reedems(deps, env, info, codes)?,
        ExecuteMsg::ClearReedems {} => execute_clean_reedems(deps, env, info)?,
    };
    Ok(res)
}

fn query_whitelist(deps: Deps) -> StdResult<WhitelistResponse> {
    let whitelist = WHITELIST.load(deps.storage)?;
    let address = whitelist.addr().to_string();
    Ok(WhitelistResponse { address })
}

fn query_is_whitelisted(deps: Deps, address: String) -> StdResult<IsWhitelistedResponse> {
    let address = deps.api.addr_validate(&address)?;
    let whitelist = WHITELIST.load(deps.storage)?;
    let whitelisted = whitelist.is_member(&deps.querier, &address)?.is_some();
    Ok(IsWhitelistedResponse { whitelisted })
}

fn query_reedem(deps: Deps, code: String) -> StdResult<ReedemResponse> {
    REEDEMS
        .may_load(deps.storage, code)
        .map(|reedem| ReedemResponse { reedem })
}

fn query_all_reedems(
    deps: Deps,
    start: Option<String>,
    limit: Option<u32>,
) -> StdResult<AllReedemsResponse> {
    let reedems = REEDEMS
        .range(
            deps.storage,
            start.map(Bound::exclusive),
            None,
            Order::Ascending,
        )
        .take(limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize)
        .map(|reedem| {
            let (code, reedem) = reedem?;
            Ok(ReedemInfo {
                code: String::from_utf8(code)?,
                sender: reedem.sender,
                amount: reedem.amount,
                memo: reedem.memo,
                timestamp: reedem.timestamp,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(AllReedemsResponse { reedems })
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Whitelist {} => to_binary(&query_whitelist(deps)?),
        QueryMsg::IsWhitelisted { address } => to_binary(&query_is_whitelisted(deps, address)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
        QueryMsg::Reedem { code } => to_binary(&query_reedem(deps, code)?),
        QueryMsg::AllReedems { start_after, limit } => {
            to_binary(&query_all_reedems(deps, start_after, limit)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{MockApi, MockStorage};
    use cosmwasm_std::{
        from_slice, ContractResult, Empty, Querier, QuerierResult, QuerierWrapper, QueryRequest,
        Storage, SystemError, SystemResult, WasmQuery,
    };
    use cw_storage_plus::Map;

    const MEMBERS: Map<&Addr, u64> = Map::new(cw4::MEMBERS_KEY);

    struct GroupQuerier {
        contract: String,
        storage: MockStorage,
    }

    impl GroupQuerier {
        pub fn new(contract: &Addr, members: &[(&Addr, u64)]) -> Self {
            let mut storage = MockStorage::new();
            for (member, weight) in members {
                MEMBERS.save(&mut storage, member, weight).unwrap();
            }
            GroupQuerier {
                contract: contract.to_string(),
                storage,
            }
        }

        fn handle_query(&self, request: QueryRequest<Empty>) -> QuerierResult {
            match request {
                QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                    self.query_wasm(contract_addr, key)
                }
                QueryRequest::Wasm(WasmQuery::Smart { .. }) => {
                    SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: "WasmQuery::Smart".to_string(),
                    })
                }
                _ => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "not wasm".to_string(),
                }),
            }
        }

        // TODO: we should be able to add a custom wasm handler to MockQuerier from cosmwasm_std::mock
        fn query_wasm(&self, contract_addr: String, key: Binary) -> QuerierResult {
            if contract_addr != self.contract {
                SystemResult::Err(SystemError::NoSuchContract {
                    addr: contract_addr,
                })
            } else {
                let bin = self.storage.get(&key).unwrap_or_default();
                SystemResult::Ok(ContractResult::Ok(bin.into()))
            }
        }
    }

    impl Querier for GroupQuerier {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            let request: QueryRequest<Empty> = match from_slice(bin_request) {
                Ok(v) => v,
                Err(e) => {
                    return SystemResult::Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })
                }
            };
            self.handle_query(request)
        }
    }

    #[test]
    // a version of multitest::whitelist_works that doesn't need multitest, App or suite
    fn whitelist_works() {
        let member = Addr::unchecked("member");
        let member2 = Addr::unchecked("member2");
        let non_member = Addr::unchecked("nonmember");

        let whitelist_addr = Addr::unchecked("whitelist");

        let querier = GroupQuerier::new(&whitelist_addr, &[(&member, 10), (&member2, 0)]);

        // set our local data
        let api = MockApi::default();
        let mut storage = MockStorage::new();
        WHITELIST
            .save(&mut storage, &Cw4Contract(whitelist_addr))
            .unwrap();
        let deps = Deps {
            storage: &storage,
            api: &api,
            querier: QuerierWrapper::new(&querier),
        };

        // sender whitelisted regardless of weight
        verify_sender_on_whitelist(deps, &member).unwrap();
        verify_sender_on_whitelist(deps, &member2).unwrap();
        let err = verify_sender_on_whitelist(deps, &non_member).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        verify_sender_and_addresses_on_whitelist(deps, &member, &[member2.as_str()]).unwrap();
    }
}
