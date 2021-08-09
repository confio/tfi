use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coin, coins, to_binary, Addr, Empty, StdError, Uint128};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};

use crate::error::ContractError;
use tfi::asset::{Asset, AssetInfo, PairInfo};
use tfi::pair::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, SimulationResponse};

fn mock_app() -> App {
    let env = mock_env();
    let api = Box::new(MockApi::default());
    let bank = BankKeeper::new();
    let storage = Box::new(MockStorage::new());

    App::new(api, env.block, bank, storage)
}

pub fn contract_pair() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply);
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

#[track_caller]
// Checks if allowances on cw20 contracts are as expected
fn assert_allowances(
    app: &App,
    contract: &Addr,
    owner: &Addr,
    allowances: Vec<cw20::AllowanceInfo>,
) {
    let nonzero_allow = |allow: &cw20::AllowanceInfo| allow.allowance != Uint128::zero();
    let allow_cmp = |l: &cw20::AllowanceInfo, r: &cw20::AllowanceInfo| l.spender.cmp(&r.spender);

    let mut allowances: Vec<_> = allowances.into_iter().filter(nonzero_allow).collect();
    allowances.sort_by(allow_cmp);

    let mut result: cw20::AllAllowancesResponse = app
        .wrap()
        .query_wasm_smart(
            contract.clone(),
            &cw20_base::msg::QueryMsg::AllAllowances {
                owner: owner.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap_or_else(|_| panic!("Query for allowances for {} on {} failed", owner, contract));

    result.allowances = result
        .allowances
        .into_iter()
        .filter(nonzero_allow)
        .collect();
    result.allowances.sort_by(allow_cmp);

    assert_eq!(result, cw20::AllAllowancesResponse { allowances });
}

#[track_caller]
// Helper function asserting proper balance on cw20 contract
fn assert_balance(app: &App, contract: &Addr, owner: &Addr, balance: u128) {
    let result: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            contract.clone(),
            &cw20_base::msg::QueryMsg::Balance {
                address: owner.to_string(),
            },
        )
        .unwrap_or_else(|_| panic!("Query for balance for {} on {} failed", owner, contract));

    assert_eq!(
        result,
        cw20::BalanceResponse {
            balance: Uint128::new(balance)
        }
    );
}

#[track_caller]
// Helper function asserting proper balance of native token
fn assert_native_balance(app: &App, denom: &str, owner: &Addr, balance: u128) {
    let result = app
        .wrap()
        .query_balance(owner.clone(), denom)
        .unwrap_or_else(|_| panic!("Query for balance of {} for {}", denom, owner));

    assert_eq!(result, coin(balance, denom));
}

#[test]
// just do basic setup
fn setup_liquidity_pool() {
    let mut app = mock_app();

    // set personal balance
    let owner = Addr::unchecked("owner");
    let init_funds = coins(20000, "btc");
    app.init_bank_balance(&owner, init_funds).unwrap();

    // set up cw20 contract with some tokens
    let cw20_id = app.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name: "Cash Money".to_string(),
        symbol: "CASH".to_string(),
        decimals: 2,
        initial_balances: vec![Cw20Coin {
            address: owner.to_string(),
            amount: Uint128::new(50000),
        }],
        mint: None,
    };
    let cash_addr = app
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH")
        .unwrap();

    // set up pair contract
    let pair_id = app.store_code(contract_pair());
    let msg = InstantiateMsg {
        asset_infos: [
            AssetInfo::Native("btc".into()),
            AssetInfo::Token(cash_addr.clone()),
        ],
        token_code_id: cw20_id,
    };
    let pair_addr = app
        .instantiate_contract(pair_id, owner.clone(), &msg, &[], "Pair")
        .unwrap();

    // run a simulate query with wrong token
    let query_msg = QueryMsg::Simulation {
        offer_asset: Asset {
            info: AssetInfo::Native("foobar".into()),
            amount: Uint128::new(1000),
        },
    };
    let err = app
        .wrap()
        .query_wasm_smart::<SimulationResponse, _, _>(&pair_addr, &query_msg)
        .unwrap_err();
    let expected_err = ContractError::AssetMismatch(AssetInfo::Native("foobar".into()).to_string());
    assert!(
        err.to_string().ends_with(&expected_err.to_string()),
        "got: {}, expected: {}",
        err.to_string(),
        expected_err.to_string()
    );

    // simulate with proper token
    let query_msg = QueryMsg::Simulation {
        offer_asset: Asset {
            info: AssetInfo::Token(cash_addr.clone()),
            amount: Uint128::new(7000),
        },
    };
    let err = app
        .wrap()
        .query_wasm_smart::<SimulationResponse, _, _>(&pair_addr, &query_msg)
        .unwrap_err();
    let expected_err = StdError::generic_err("Divide by zero error computing the swap");
    assert!(
        err.to_string().ends_with(&expected_err.to_string()),
        "got: {}, expected: {}",
        err.to_string(),
        expected_err.to_string()
    );

    // provide an allowance to pay into LP
    // let cash = Cw20Contract(cash_addr.clone());
    let allow_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: pair_addr.to_string(),
        amount: Uint128::new(10000),
        expires: None,
    };
    let _ = app
        .execute_contract(owner.clone(), cash_addr.clone(), &allow_msg, &[])
        .unwrap();

    // provide liquidity with proper tokens
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Native("btc".into()),
                amount: Uint128::new(10),
            },
            Asset {
                info: AssetInfo::Token(cash_addr),
                amount: Uint128::new(7000),
            },
        ],
        slippage_tolerance: None,
    };
    let _ = app
        .execute_contract(owner, pair_addr.clone(), &msg, &coins(10, "btc"))
        .unwrap();

    // simulate again
    let res: SimulationResponse = app.wrap().query_wasm_smart(&pair_addr, &query_msg).unwrap();
    // doubling the amount of cash should return half the BTC from the LP
    assert_eq!(res.return_amount, Uint128::new(5));

    // TODO: actually perform swap and check value
}

#[test]
// Simple swap scenario
//
// * Create (token, native token) pair
// * Give allowance for pair
// * Provide liquidity to pair
// * Validate proper allowance on token as well as tokens posession
// * Perform single swap of native tokens to tokens
// * Verify proper amount of tokens (including 0.3% fee)
// * Perform single swap of tokens to native tokens
// * Verify proper amount of tokens (including 0.3% fee)
// * Withdraw liquidity, all fees should be also added
fn single_swap() {
    let mut app = mock_app();

    let cw20_id = app.store_code(contract_cw20());
    let pair_id = app.store_code(contract_pair());

    // Initialize actors:
    // cash: cw20 contract
    // pair: tfi-pair contract between btc (native) and cash (cw20)
    // liquidity token (lt): cw20 contract for pair liquidity tokens
    // liquidity provider (lp): 2000btc + 6000cash
    // trader: 1000btc
    // trader_recv: nothing (should receive funds later)
    let lp = Addr::unchecked("liquidity-provider");
    app.init_bank_balance(&lp, coins(2000, "btc")).unwrap();

    let trader = Addr::unchecked("trader");
    app.init_bank_balance(&trader, coins(1000, "btc")).unwrap();

    let trader_recv = Addr::unchecked("trader-recv");

    let cash = app
        .instantiate_contract(
            cw20_id,
            lp.clone(),
            &cw20_base::msg::InstantiateMsg {
                name: "Cash Money".to_owned(),
                symbol: "cash".to_owned(),
                decimals: 2,
                initial_balances: vec![Cw20Coin {
                    address: lp.to_string(),
                    amount: Uint128::new(6000),
                }],
                mint: None,
            },
            &[],
            "Cash",
        )
        .unwrap();

    let pair = app
        .instantiate_contract(
            pair_id,
            lp.clone(),
            &InstantiateMsg {
                asset_infos: [
                    AssetInfo::Native("btc".to_owned()),
                    AssetInfo::Token(cash.clone()),
                ],
                token_code_id: cw20_id,
            },
            &[],
            "Pair",
        )
        .unwrap();

    let PairInfo {
        liquidity_token: lt,
        ..
    } = app
        .wrap()
        .query_wasm_smart(pair.clone(), &QueryMsg::Pair {})
        .unwrap();

    // Provide allowance
    app.execute_contract(
        lp.clone(),
        cash.clone(),
        &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
            spender: pair.to_string(),
            amount: Uint128::new(6000),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Provide liquidity
    app.execute_contract(
        lp.clone(),
        pair.clone(),
        &ExecuteMsg::ProvideLiquidity {
            assets: [
                Asset {
                    info: AssetInfo::Native("btc".to_owned()),
                    amount: Uint128::new(2000),
                },
                Asset {
                    info: AssetInfo::Token(cash.clone()),
                    amount: Uint128::new(6000),
                },
            ],
            slippage_tolerance: None,
        },
        &coins(2000, "btc"),
    )
    .unwrap();

    // liquidity provider --> pair: 6000btc + 2000cash
    // liquidity provider: 3464lt minted by pair (provided sqrt(6000 [btc] * 2000 [cash])
    assert_native_balance(&app, "btc", &lp, 0);
    assert_native_balance(&app, "btc", &trader, 1000);
    assert_native_balance(&app, "btc", &trader_recv, 0);
    assert_native_balance(&app, "btc", &pair, 2000);
    assert_balance(&app, &cash, &lp, 0);
    assert_balance(&app, &cash, &trader, 0);
    assert_balance(&app, &cash, &trader_recv, 0);
    assert_balance(&app, &cash, &pair, 6000);
    assert_balance(&app, &lt, &lp, 3464);
    assert_balance(&app, &lt, &trader, 0);
    assert_balance(&app, &lt, &trader_recv, 0);
    assert_balance(&app, &lt, &pair, 0);
    assert_allowances(&app, &cash, &lp, vec![]);

    // Swap btc for cash
    app.execute_contract(
        trader.clone(),
        pair.clone(),
        &ExecuteMsg::Swap {
            offer_asset: Asset {
                info: AssetInfo::Native("btc".to_owned()),
                amount: Uint128::new(1000),
            },
            belief_price: None,
            max_spread: None,
            to: None,
        },
        &coins(1000, "btc"),
    )
    .unwrap();

    // trader -> pair: 1000btc
    // pair -> trader: 1994cash, explanaction:
    //   btc to be left on contract: 6000 * 2000 / (2000 + 1000) = 4000
    //   btc to be paid out: 6000 - 2000 = 4000
    //   btc to be paid out after commission: 2000 - 2000 * 0.03% = 2000 - 2000 * 0.997 = 1994
    assert_native_balance(&app, "btc", &lp, 0);
    assert_native_balance(&app, "btc", &trader, 0);
    assert_native_balance(&app, "btc", &trader_recv, 0);
    assert_native_balance(&app, "btc", &pair, 3000);
    assert_balance(&app, &cash, &lp, 0);
    assert_balance(&app, &cash, &trader, 1994);
    assert_balance(&app, &cash, &trader_recv, 0);
    assert_balance(&app, &cash, &pair, 4006);
    assert_balance(&app, &lt, &lp, 3464);
    assert_balance(&app, &lt, &trader, 0);
    assert_balance(&app, &lt, &trader_recv, 0);
    assert_balance(&app, &lt, &pair, 0);
    assert_allowances(&app, &cash, &lp, vec![]);

    // Swap cash for btc
    app.execute_contract(
        trader.clone(),
        cash.clone(),
        &cw20_base::msg::ExecuteMsg::Send {
            contract: pair.to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&Cw20HookMsg::Swap {
                belief_price: None,
                max_spread: None,
                to: Some(trader_recv.to_string()),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    // trader -> pair: 1000cash
    // pair -> trader_recv: 599 cash, explanation:
    //   cash to be left on contract: 3000 * 4006 / (4006 + 1000) = 2400
    //   cash to be paid out: 3000 - 2400 = 600
    //   cash to be paid out after commission: 600 - 600 * 0.003 = 599
    assert_native_balance(&app, "btc", &lp, 0);
    assert_native_balance(&app, "btc", &trader, 0);
    assert_native_balance(&app, "btc", &trader_recv, 599);
    assert_native_balance(&app, "btc", &pair, 2401);
    assert_balance(&app, &cash, &lp, 0);
    assert_balance(&app, &cash, &trader, 994);
    assert_balance(&app, &cash, &trader_recv, 0);
    assert_balance(&app, &cash, &pair, 5006);
    assert_balance(&app, &lt, &lp, 3464);
    assert_balance(&app, &lt, &trader, 0);
    assert_balance(&app, &lt, &trader_recv, 0);
    assert_balance(&app, &lt, &pair, 0);
    assert_allowances(&app, &cash, &lp, vec![]);

    // Withdraw liqidity
    app.execute_contract(
        lp.clone(),
        lt.clone(),
        &cw20_base::msg::ExecuteMsg::Send {
            contract: pair.to_string(),
            amount: Uint128::new(3464),
            msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // liquidity provider -> pair: 3464lt (all burned in pair)
    // pair -> liquidity provider: 2401btc + 5006cash (whole pair - lp owned 100% of lt)
    //
    // Note, that lp provided initially 6000btc and 2000cash, 6000 * 2000 = 12*10^6
    // Lp payed out 2401btc, and 5006 cash, 2401 * 5006 > 12 * 10^6
    // 1btc and 6cash is what lp earned on commissions, as 2400 * 5000 = 12*10^6
    assert_native_balance(&app, "btc", &lp, 2401);
    assert_native_balance(&app, "btc", &trader, 0);
    assert_native_balance(&app, "btc", &trader_recv, 599);
    assert_native_balance(&app, "btc", &pair, 0);
    assert_balance(&app, &cash, &lp, 5006);
    assert_balance(&app, &cash, &trader, 994);
    assert_balance(&app, &cash, &trader_recv, 0);
    assert_balance(&app, &cash, &pair, 0);
    assert_balance(&app, &lt, &lp, 0);
    assert_balance(&app, &lt, &trader, 0);
    assert_balance(&app, &lt, &trader_recv, 0);
    assert_balance(&app, &lt, &pair, 0);
    assert_allowances(&app, &cash, &lp, vec![]);
}
