use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, Event, MessageInfo, Order, Response,
    StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20_base::allowances::query_allowance;
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};
use cw20_base::enumerable::{query_all_accounts, query_owner_allowances};
use cw20_base::state::{BALANCES, TOKEN_INFO};
use cw20_base::ContractError as Cw20ContractError;
use cw_storage_plus::Bound;
use tg4::Tg4Contract;

use crate::error::ContractError;
use crate::msg::{
    AllRedeemsResponse, ExecuteMsg, InstantiateMsg, IsWhitelistedResponse, QueryMsg, RedeemInfo,
    RedeemResponse, WhitelistResponse,
};
use crate::state::{Redeem, REEDEMS, WHITELIST};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:trusted-token";
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
        name: msg.name.clone(),
        symbol: msg.symbol.clone(),
        decimals: msg.decimals,
        initial_balances: msg.initial_balances,
        mint: msg.mint,
        marketing: msg.marketing,
    };
    cw20_base::contract::instantiate(deps.branch(), env, info, cw20_msg)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let addr = deps.api.addr_validate(&msg.whitelist_group)?;
    let contract = Tg4Contract(addr.clone());
    // verify that the whitelist contract is actually tg4-compatible
    contract.list_members(&deps.querier, None, Some(1))?;
    WHITELIST.save(deps.storage, &contract)?;

    let event = Event::new("create_token")
        .add_attribute("name", msg.name)
        .add_attribute("symbol", msg.symbol)
        .add_attribute("decimal", msg.decimals.to_string())
        .add_attribute("allow_group", addr.to_string());
    Ok(Response::default().add_event(event))
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
        let validated_address = deps.api.addr_validate(address)?;
        if whitelist
            .is_member(&deps.querier, &validated_address)?
            .is_none()
        {
            return Err(ContractError::Unauthorized {});
        }
    }
    Ok(())
}

/// Redeems token effectively burning them and storing information about redeem internally. This
/// also triggers custom `redeem` event with details of process. Before redeeming, sender should
/// make sure, that token provider is aware about such possibility and is willing to cover redeem
/// off-chain, otherwise this may be equivalent to destrotying commodity.
fn execute_redeem(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    code: String,
    sender: Option<String>,
    memo: String,
) -> Result<Response, ContractError> {
    if REEDEMS.has(deps.storage, &code) {
        return Err(ContractError::RedeemCodeUsed {});
    }

    if amount == Uint128::zero() {
        return Err(Cw20ContractError::InvalidZeroAmount {}.into());
    }

    // lower balance
    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> Result<_, ContractError> {
            let balance = balance.unwrap_or_default();
            balance
                .checked_sub(amount)
                .map_err(|_| ContractError::RedeemOverBalance(balance))
        },
    )?;

    // reduce total_supply
    TOKEN_INFO.update(deps.storage, |mut info| -> StdResult<_> {
        info.total_supply = info.total_supply.checked_sub(amount)?;
        Ok(info)
    })?;

    REEDEMS.save(
        deps.storage,
        &code,
        &Redeem {
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

    let event = Event::new("redeem")
        .add_attribute("code", code)
        .add_attribute("sender", sender)
        .add_attribute("amount", amount)
        .add_attribute("memo", memo);

    Ok(Response::new()
        .add_event(event)
        .add_attribute("action", "redeem")
        .add_attribute("from", info.sender)
        .add_attribute("amount", amount))
}

/// Removes info about redeems from contract, can be performed by minter only
fn execute_remove_redeems(
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
        REEDEMS.remove(deps.storage, &code);
    }

    Ok(Response::new().add_attribute("action", "remove_redeems"))
}

/// Removes all redeems info from contract
fn execute_clean_redeems(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = TOKEN_INFO.load(deps.storage)?;
    if config.mint.is_none() || config.mint.as_ref().unwrap().minter != info.sender {
        return Err(Cw20ContractError::Unauthorized {}.into());
    }

    let keys = REEDEMS
        .keys(deps.storage, None, None, Order::Ascending)
        .collect::<Result<Vec<String>, StdError>>()?;

    for key in keys {
        REEDEMS.remove(deps.storage, &key)
    }

    Ok(Response::new().add_attribute("action", "remove_all_redeems"))
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
        ExecuteMsg::Redeem {
            amount,
            code,
            sender,
            memo,
        } => execute_redeem(deps, env, info, amount, code, sender, memo)?,
        ExecuteMsg::RemoveRedeems { codes } => execute_remove_redeems(deps, env, info, codes)?,
        ExecuteMsg::ClearRedeems {} => execute_clean_redeems(deps, env, info)?,
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

fn query_redeem(deps: Deps, code: String) -> StdResult<RedeemResponse> {
    REEDEMS
        .may_load(deps.storage, &code)
        .map(|redeem| RedeemResponse { redeem })
}

fn query_all_redeems(
    deps: Deps,
    start: Option<String>,
    limit: Option<u32>,
) -> StdResult<AllRedeemsResponse> {
    let redeems = REEDEMS
        .range(
            deps.storage,
            start.as_ref().map(|s| Bound::exclusive(s.as_str())),
            None,
            Order::Ascending,
        )
        .take(limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize)
        .map(|redeem| {
            let (code, redeem) = redeem?;
            Ok(RedeemInfo {
                code,
                sender: redeem.sender,
                amount: redeem.amount,
                memo: redeem.memo,
                timestamp: redeem.timestamp,
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(AllRedeemsResponse { redeems })
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
        } => to_binary(&query_owner_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
        QueryMsg::Redeem { code } => to_binary(&query_redeem(deps, code)?),
        QueryMsg::AllRedeems { start_after, limit } => {
            to_binary(&query_all_redeems(deps, start_after, limit)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage};
    use cosmwasm_std::{
        from_binary, from_slice, ContractResult, Empty, OwnedDeps, Querier, QuerierResult,
        QuerierWrapper, QueryRequest, Storage, SystemError, SystemResult, WasmQuery,
    };
    use cw20_base::state::TokenInfo;
    use cw_storage_plus::Map;
    use tg4::{MemberInfo, MemberListResponse, Tg4QueryMsg};

    use std::marker::PhantomData;

    const MEMBERS: Map<&Addr, MemberInfo> = Map::new(tg4::MEMBERS_KEY);

    struct GroupQuerier {
        contract: String,
        storage: MockStorage,
    }

    impl GroupQuerier {
        pub fn new(contract: &Addr, members: &[(&Addr, u64)]) -> Self {
            let mut storage = MockStorage::new();
            for (member, weight) in members {
                MEMBERS
                    .save(&mut storage, member, &MemberInfo::new(*weight))
                    .unwrap();
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
                QueryRequest::Wasm(WasmQuery::Smart { msg, .. }) => self.query_wasm_smart(msg),
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

        fn query_wasm_smart(&self, msg: Binary) -> QuerierResult {
            match from_binary(&msg) {
                Ok(Tg4QueryMsg::ListMembers { .. }) => {
                    let mlr = MemberListResponse { members: vec![] };
                    SystemResult::Ok(ContractResult::Ok(to_binary(&mlr).unwrap()))
                }
                _ => SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "Not ListMembers query".to_string(),
                }),
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
            .save(&mut storage, &Tg4Contract(whitelist_addr))
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

    #[test]
    fn redeem_over_balance() {
        let trader = Addr::unchecked("trader");
        let whitelist_addr = Addr::unchecked("whitelist");

        let querier = GroupQuerier::new(&whitelist_addr, &[]);

        // set our local data
        let api = MockApi::default();
        let mut storage = MockStorage::new();

        WHITELIST
            .save(&mut storage, &Tg4Contract(whitelist_addr))
            .unwrap();

        let mut deps = OwnedDeps {
            storage,
            api,
            querier,
            custom_query_type: PhantomData,
        };

        BALANCES
            .save(&mut deps.storage, &trader, &Uint128::new(100))
            .unwrap();

        TOKEN_INFO
            .save(
                &mut deps.storage,
                &TokenInfo {
                    name: "Token".to_owned(),
                    symbol: "TKN".to_owned(),
                    decimals: 4,
                    total_supply: Uint128::new(100),
                    mint: None,
                },
            )
            .unwrap();

        let err = execute_redeem(
            deps.as_mut(),
            mock_env(),
            mock_info(&trader.to_string(), &[]),
            Uint128::new(500),
            "redeem-code".to_owned(),
            None,
            "Redeem description".to_owned(),
        )
        .unwrap_err();

        assert_eq!(ContractError::RedeemOverBalance(Uint128::new(100)), err);
    }

    #[test]
    fn redeem_empty_account() {
        let trader = Addr::unchecked("trader");
        let whitelist_addr = Addr::unchecked("whitelist");

        let querier = GroupQuerier::new(&whitelist_addr, &[]);

        // set our local data
        let api = MockApi::default();
        let mut storage = MockStorage::new();

        WHITELIST
            .save(&mut storage, &Tg4Contract(whitelist_addr))
            .unwrap();

        let mut deps = OwnedDeps {
            storage,
            api,
            querier,
            custom_query_type: PhantomData,
        };

        TOKEN_INFO
            .save(
                &mut deps.storage,
                &TokenInfo {
                    name: "Token".to_owned(),
                    symbol: "TKN".to_owned(),
                    decimals: 4,
                    total_supply: Uint128::zero(),
                    mint: None,
                },
            )
            .unwrap();

        let err = execute_redeem(
            deps.as_mut(),
            mock_env(),
            mock_info(&trader.to_string(), &[]),
            Uint128::new(500),
            "redeem-code".to_owned(),
            None,
            "Redeem description".to_owned(),
        )
        .unwrap_err();

        assert_eq!(ContractError::RedeemOverBalance(Uint128::new(0)), err);
    }

    #[test]
    fn redeem_already_used_code() {
        let trader = Addr::unchecked("trader");
        let whitelist_addr = Addr::unchecked("whitelist");

        let querier = GroupQuerier::new(&whitelist_addr, &[]);

        // set our local data
        let api = MockApi::default();
        let mut storage = MockStorage::new();

        WHITELIST
            .save(&mut storage, &Tg4Contract(whitelist_addr))
            .unwrap();

        let mut deps = OwnedDeps {
            storage,
            api,
            querier,
            custom_query_type: PhantomData,
        };

        BALANCES
            .save(&mut deps.storage, &trader, &Uint128::new(100))
            .unwrap();

        TOKEN_INFO
            .save(
                &mut deps.storage,
                &TokenInfo {
                    name: "Token".to_owned(),
                    symbol: "TKN".to_owned(),
                    decimals: 4,
                    total_supply: Uint128::new(100),
                    mint: None,
                },
            )
            .unwrap();

        execute_redeem(
            deps.as_mut(),
            mock_env(),
            mock_info(&trader.to_string(), &[]),
            Uint128::new(50),
            "redeem-code".to_owned(),
            None,
            "Redeem description".to_owned(),
        )
        .unwrap();

        let err = execute_redeem(
            deps.as_mut(),
            mock_env(),
            mock_info(&trader.to_string(), &[]),
            Uint128::new(30),
            "redeem-code".to_owned(),
            None,
            "Another redeem description".to_owned(),
        )
        .unwrap_err();

        assert_eq!(ContractError::RedeemCodeUsed {}, err);
    }

    #[test]
    fn instantiate_event() {
        let name = "Liquid Gold".to_string();
        let symbol = "GOLD".to_string();
        let decimals = 6;
        let whitelist_group = "tgrade123456789".to_string();
        let instantiate_msg = InstantiateMsg {
            name: name.clone(),
            symbol: symbol.clone(),
            decimals,
            initial_balances: vec![],
            mint: None,
            marketing: None,
            whitelist_group: whitelist_group.clone(),
        };

        let whitelist_addr = Addr::unchecked("whitelist");
        let mut deps = OwnedDeps {
            storage: MockStorage::new(),
            api: MockApi::default(),
            querier: GroupQuerier::new(&whitelist_addr, &[]),
            custom_query_type: PhantomData::<Empty>,
        };

        let info = MessageInfo {
            sender: Addr::unchecked("SENDER"),
            funds: vec![],
        };

        assert_eq!(
            instantiate(deps.as_mut(), mock_env(), info, instantiate_msg),
            Ok(Response::new().add_event(
                Event::new("create_token")
                    .add_attribute("name", name)
                    .add_attribute("symbol", symbol)
                    .add_attribute("decimal", decimals.to_string())
                    .add_attribute("allow_group", whitelist_group)
            ))
        );
    }
}
