use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coins, Addr, Empty, StdError, Uint128};
use cw20::Cw20Coin;
use cw_multi_test::{App, Contract, ContractWrapper, SimpleBank};

use crate::error::ContractError;
use tfi::asset::{Asset, AssetInfo};
use tfi::pair::{ExecuteMsg, InstantiateMsg, QueryMsg, SimulationResponse};

fn mock_app() -> App {
    let env = mock_env();
    let api = Box::new(MockApi::default());
    let bank = SimpleBank {};

    App::new(api, env.block, bank, || Box::new(MockStorage::new()))
}

pub fn contract_pair() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
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

#[test]
// just do basic setup
fn setup_liquidity_pool() {
    let mut app = mock_app();

    // set personal balance
    let owner = Addr::unchecked("owner");
    let init_funds = coins(20000, "btc");
    app.set_bank_balance(&owner, init_funds).unwrap();

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

    // set up cw20 helpers
    // let cash = Cw20Contract(cash_addr.clone());

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

    // provide liquidity with proper tokens
    let msg = ExecuteMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Native("btc".into()),
                amount: Uint128::new(10),
            },
            Asset {
                info: AssetInfo::Token(cash_addr.clone()),
                amount: Uint128(7000),
            },
        ],
        slippage_tolerance: None,
    };
    let _ = app
        .execute_contract(owner.clone(), pair_addr.clone(), &msg, &coins(10, "btc"))
        .unwrap_err();

    // // simulate again
    // let res: SimulationResponse = app.wrap().query_wasm_smart(&pair_addr, &query_msg).unwrap();
    // // doubling the amount of cash should return half the BTC from the LP
    // assert_eq!(res.return_amount, Uint128::new(5));

    // // send some tokens to create an escrow
    // let arb = Addr::unchecked("arbiter");
    // let ben = String::from("beneficiary");
    // let id = "demo".to_string();
    // let create_msg = ReceiveMsg::Create(CreateMsg {
    //     id: id.clone(),
    //     arbiter: arb.to_string(),
    //     recipient: ben.clone(),
    //     end_height: None,
    //     end_time: None,
    //     cw20_whitelist: None,
    // });
    // let send_msg = Cw20ExecuteMsg::Send {
    //     contract: escrow_addr.to_string(),
    //     amount: Uint128::new(1200),
    //     msg: to_binary(&create_msg).unwrap(),
    // };
    // let res = router
    //     .execute_contract(owner.clone(), cash_addr.clone(), &send_msg, &[])
    //     .unwrap();
    // assert_eq!(2, res.events.len());
    // println!("{:?}", res.events);
    // let cw20_attr = res.custom_attrs(0);
    // println!("{:?}", cw20_attr);
    // assert_eq!(4, cw20_attr.len());
    // let escrow_attr = res.custom_attrs(1);
    // println!("{:?}", escrow_attr);
    // assert_eq!(2, escrow_attr.len());
}
