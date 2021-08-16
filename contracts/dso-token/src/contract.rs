use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use cw20_base::allowances::query_allowance;
use cw20_base::contract::{
    query_balance, query_download_logo, query_marketing_info, query_minter, query_token_info,
};
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};
use cw4::Cw4Contract;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, IsWhitelistedResponse, QueryMsg, WhitelistResponse};
use crate::state::WHITELIST;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:dso-token";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

fn verify_sender_on_whitelist(deps: &DepsMut, sender: &Addr) -> Result<(), ContractError> {
    let whitelist = WHITELIST.load(deps.storage)?;
    if whitelist.is_member(&deps.querier, sender)?.is_none() {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn verify_sender_and_addresses_on_whitelist(
    deps: &DepsMut,
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

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let res = match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            verify_sender_and_addresses_on_whitelist(&deps, &info.sender, &[&recipient])?;
            cw20_base::contract::execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => {
            verify_sender_on_whitelist(&deps, &info.sender)?;
            cw20_base::contract::execute_burn(deps, env, info, amount)
        }
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => {
            verify_sender_and_addresses_on_whitelist(&deps, &info.sender, &[&contract])?;
            cw20_base::contract::execute_send(deps, env, info, contract, amount, msg)
        }
        ExecuteMsg::Mint { recipient, amount } => {
            verify_sender_and_addresses_on_whitelist(&deps, &info.sender, &[&recipient])?;
            cw20_base::contract::execute_mint(deps, env, info, recipient, amount)
        }
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => {
            verify_sender_on_whitelist(&deps, &info.sender)?;
            cw20_base::allowances::execute_increase_allowance(
                deps, env, info, spender, amount, expires,
            )
        }
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => {
            verify_sender_on_whitelist(&deps, &info.sender)?;
            cw20_base::allowances::execute_decrease_allowance(
                deps, env, info, spender, amount, expires,
            )
        }
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => {
            verify_sender_and_addresses_on_whitelist(&deps, &info.sender, &[&owner, &recipient])?;
            cw20_base::allowances::execute_transfer_from(deps, env, info, owner, recipient, amount)
        }
        ExecuteMsg::BurnFrom { owner, amount } => {
            verify_sender_and_addresses_on_whitelist(&deps, &info.sender, &[&owner])?;
            cw20_base::allowances::execute_burn_from(deps, env, info, owner, amount)
        }
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => {
            verify_sender_and_addresses_on_whitelist(&deps, &info.sender, &[&owner, &contract])?;
            cw20_base::allowances::execute_send_from(deps, env, info, owner, contract, amount, msg)
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
        ),
        ExecuteMsg::UploadLogo(logo) => {
            cw20_base::contract::execute_upload_logo(deps, env, info, logo)
        }
    };
    Ok(res?)
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
    }
}
