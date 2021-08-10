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

#[cfg(test)]
mod tests {
    use cw20_base::msg::InstantiateMarketingInfo;

    use super::*;

    use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
    use cosmwasm_std::{coins, Addr, Coin, Empty, Uint128};
    use cw20::{Cw20Coin, Cw20Contract, TokenInfoResponse};
    use cw4::Member;
    use cw_multi_test::{next_block, App, BankKeeper, Contract, ContractWrapper, Executor};

    fn mock_app() -> App {
        let env = mock_env();
        let api = MockApi::default();
        let bank = BankKeeper::new();
        let storage = MockStorage::new();

        App::new(api, env.block, bank, storage)
    }

    pub fn contract_group() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            cw4_group::contract::execute,
            cw4_group::contract::instantiate,
            cw4_group::contract::query,
        );
        Box::new(contract)
    }

    pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        );
        Box::new(contract)
    }

    const OWNER: &str = "owner";
    const MEMBER1: &str = "member1";
    const MEMBER2: &str = "member2";
    const NON_MEMBER: &str = "non-member";

    #[derive(Default)]
    struct SuiteConfig {
        init_funds: Vec<Coin>,
        initial_balances: Vec<Cw20Coin>,
        marketing: Option<InstantiateMarketingInfo>,
    }

    impl SuiteConfig {
        fn init(self) -> Suite {
            Suite::init(self)
        }
    }

    struct Suite {
        router: App,
        group_addr: Addr,
        cw20_addr: Addr,
        cash: Cw20Contract,
    }

    impl Suite {
        fn init(config: SuiteConfig) -> Self {
            let mut router = mock_app();

            // set personal balance
            let owner = Addr::unchecked(OWNER);
            router.init_bank_balance(&owner, config.init_funds).unwrap();

            // create group contract
            let group_id = router.store_code(contract_group());
            let msg = cw4_group::msg::InstantiateMsg {
                admin: Some(OWNER.to_owned()),
                members: vec![
                    Member {
                        addr: MEMBER1.to_owned(),
                        weight: 50,
                    },
                    Member {
                        addr: MEMBER2.to_owned(),
                        weight: 0,
                    },
                ],
            };
            let group_addr = router
                .instantiate_contract(group_id, owner.clone(), &msg, &[], "WHITELIST", None)
                .unwrap();
            router.update_block(next_block);

            let cw20_id = router.store_code(contract_cw20());
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_owned(),
                symbol: "CASH".to_owned(),
                decimals: 9,
                initial_balances: config.initial_balances,
                mint: None,
                marketing: config.marketing,
                whitelist_group: group_addr.to_string(),
            };
            let cw20_addr = router
                .instantiate_contract(cw20_id, owner, &instantiate_msg, &[], "CASH", None)
                .unwrap();
            router.update_block(next_block);

            let cash = Cw20Contract(cw20_addr.clone());

            Suite {
                router,
                group_addr,
                cw20_addr,
                cash,
            }
        }
    }

    #[test]
    fn proper_instantiation() {
        let amount = Uint128::from(11223344u128);
        let Suite { cash, router, .. } = SuiteConfig {
            init_funds: coins(2000, "btc"),
            initial_balances: vec![
                Cw20Coin {
                    address: String::from(MEMBER1),
                    amount,
                },
                Cw20Coin {
                    address: String::from(OWNER),
                    amount,
                },
            ],
            ..SuiteConfig::default()
        }
        .init();

        assert_eq!(
            cash.meta(&router).unwrap(),
            TokenInfoResponse {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                total_supply: amount + amount,
            }
        );
        assert_eq!(cash.balance(&router, MEMBER1).unwrap(), amount);
        assert_eq!(cash.balance(&router, MEMBER2).unwrap(), Uint128::zero());
    }

    #[test]
    fn transfer() {
        let amount = Uint128::from(11223344u128);
        let Suite {
            cash,
            mut router,
            cw20_addr,
            ..
        } = SuiteConfig {
            init_funds: coins(2000, "btc"),
            initial_balances: vec![
                Cw20Coin {
                    address: String::from(MEMBER1),
                    amount,
                },
                Cw20Coin {
                    address: String::from(OWNER),
                    amount,
                },
            ],
            ..SuiteConfig::default()
        }
        .init();

        // send to whitelisted member works
        let to_send = Uint128::new(50000);
        let good_send = ExecuteMsg::Transfer {
            recipient: MEMBER2.into(),
            amount: to_send,
        };
        router
            .execute_contract(Addr::unchecked(MEMBER1), cw20_addr.clone(), &good_send, &[])
            .unwrap();
        assert_eq!(
            cash.balance(&router, MEMBER1).unwrap(),
            amount.checked_sub(to_send).unwrap()
        );
        assert_eq!(cash.balance(&router, MEMBER2).unwrap(), to_send);

        // send to non-whitelisted address fails
        let bad_send = ExecuteMsg::Transfer {
            recipient: NON_MEMBER.into(),
            amount: to_send,
        };
        let err = router
            .execute_contract(Addr::unchecked(MEMBER1), cw20_addr, &bad_send, &[])
            .unwrap_err();
        assert_eq!(&err, "Unauthorized");
    }

    #[test]
    fn whitelist() {
        let Suite {
            router,
            cw20_addr,
            group_addr,
            ..
        } = SuiteConfig::default().init();

        let whitelist: WhitelistResponse = router
            .wrap()
            .query_wasm_smart(&cw20_addr, &QueryMsg::Whitelist {})
            .unwrap();
        assert_eq!(whitelist.address, group_addr);

        let is_whitelisted: IsWhitelistedResponse = router
            .wrap()
            .query_wasm_smart(
                &cw20_addr,
                &QueryMsg::IsWhitelisted {
                    address: MEMBER1.to_owned(),
                },
            )
            .unwrap();
        assert!(is_whitelisted.whitelisted);

        let is_whitelisted: IsWhitelistedResponse = router
            .wrap()
            .query_wasm_smart(
                &cw20_addr,
                &QueryMsg::IsWhitelisted {
                    address: NON_MEMBER.to_owned(),
                },
            )
            .unwrap();
        assert!(!is_whitelisted.whitelisted);
    }

    #[test]
    fn update_marketing() {
        let Suite {
            mut router,
            cw20_addr,
            ..
        } = SuiteConfig {
            marketing: Some(InstantiateMarketingInfo {
                project: None,
                description: None,
                marketing: Some(MEMBER1.to_owned()),
                logo: None,
            }),
            ..SuiteConfig::default()
        }
        .init();

        router
            .execute_contract(
                Addr::unchecked(MEMBER1),
                cw20_addr.clone(),
                &ExecuteMsg::UpdateMarketing {
                    project: Some("Project".to_owned()),
                    description: None,
                    marketing: Some(NON_MEMBER.to_owned()),
                },
                &[],
            )
            .unwrap();

        let marketing: cw20::MarketingInfoResponse = router
            .wrap()
            .query_wasm_smart(&cw20_addr, &QueryMsg::MarketingInfo {})
            .unwrap();

        assert_eq!(
            marketing,
            cw20::MarketingInfoResponse {
                project: Some("Project".to_owned()),
                description: None,
                marketing: Some(Addr::unchecked(NON_MEMBER)),
                logo: None,
            }
        );

        router
            .execute_contract(
                Addr::unchecked(NON_MEMBER),
                cw20_addr.clone(),
                &ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: Some("Description".to_owned()),
                    marketing: None,
                },
                &[],
            )
            .unwrap();

        let marketing: cw20::MarketingInfoResponse = router
            .wrap()
            .query_wasm_smart(&cw20_addr, &QueryMsg::MarketingInfo {})
            .unwrap();

        assert_eq!(
            marketing,
            cw20::MarketingInfoResponse {
                project: Some("Project".to_owned()),
                description: Some("Description".to_owned()),
                marketing: Some(Addr::unchecked(NON_MEMBER)),
                logo: None,
            }
        );
    }

    #[test]
    fn update_logo() {
        let Suite {
            mut router,
            cw20_addr,
            ..
        } = SuiteConfig {
            marketing: Some(InstantiateMarketingInfo {
                project: None,
                description: None,
                marketing: Some(MEMBER1.to_owned()),
                logo: None,
            }),
            ..SuiteConfig::default()
        }
        .init();

        let logo_url = "https://logo.url/logo.svg".to_owned();

        router
            .execute_contract(
                Addr::unchecked(MEMBER1),
                cw20_addr.clone(),
                &ExecuteMsg::UploadLogo(cw20::Logo::Url(logo_url.clone())),
                &[],
            )
            .unwrap();

        let marketing: cw20::MarketingInfoResponse = router
            .wrap()
            .query_wasm_smart(&cw20_addr, &QueryMsg::MarketingInfo {})
            .unwrap();

        assert_eq!(
            marketing,
            cw20::MarketingInfoResponse {
                project: None,
                description: None,
                marketing: Some(Addr::unchecked(MEMBER1)),
                logo: Some(cw20::LogoInfo::Url(logo_url)),
            }
        );

        router
            .execute_contract(
                Addr::unchecked(MEMBER1),
                cw20_addr.clone(),
                &ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some(NON_MEMBER.to_owned()),
                },
                &[],
            )
            .unwrap();

        let logo_url = "https://logo.url/updated_logo.svg".to_owned();

        router
            .execute_contract(
                Addr::unchecked(NON_MEMBER),
                cw20_addr.clone(),
                &ExecuteMsg::UploadLogo(cw20::Logo::Url(logo_url.clone())),
                &[],
            )
            .unwrap();

        let marketing: cw20::MarketingInfoResponse = router
            .wrap()
            .query_wasm_smart(&cw20_addr, &QueryMsg::MarketingInfo {})
            .unwrap();

        assert_eq!(
            marketing,
            cw20::MarketingInfoResponse {
                project: None,
                description: None,
                marketing: Some(Addr::unchecked(NON_MEMBER)),
                logo: Some(cw20::LogoInfo::Url(logo_url)),
            }
        );
    }
}
