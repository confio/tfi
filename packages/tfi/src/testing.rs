use crate::asset::{send_asset, Asset, AssetInfo, PairInfo};
use crate::mock_querier::mock_dependencies;
use crate::querier::{
    query_all_balances, query_balance, query_pair_info, query_supply, query_token_balance,
};

use cosmwasm_std::testing::MOCK_CONTRACT_ADDR;
use cosmwasm_std::{coins, to_binary, Addr, BankMsg, Coin, CosmosMsg, Uint128, WasmMsg};
use cw20::{Cw20CoinVerified, Cw20ExecuteMsg};

#[test]
fn token_balance_querier() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_token_balances(&[(
        &"liquidity0000".to_string(),
        &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128(123u128))],
    )]);

    assert_eq!(
        Uint128(123u128),
        query_token_balance(
            &deps.as_ref().querier,
            Addr::unchecked("liquidity0000"),
            Addr::unchecked(MOCK_CONTRACT_ADDR),
        )
        .unwrap()
    );
}

#[test]
fn balance_querier() {
    let deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128(200u128),
    }]);

    assert_eq!(
        query_balance(
            &deps.as_ref().querier,
            Addr::unchecked(MOCK_CONTRACT_ADDR),
            "uusd".to_string()
        )
        .unwrap(),
        Uint128(200u128)
    );
}

#[test]
fn all_balances_querier() {
    let deps = mock_dependencies(&[
        Coin {
            denom: "uusd".to_string(),
            amount: Uint128(200u128),
        },
        Coin {
            denom: "ukrw".to_string(),
            amount: Uint128(300u128),
        },
    ]);

    assert_eq!(
        query_all_balances(&deps.as_ref().querier, Addr::unchecked(MOCK_CONTRACT_ADDR),).unwrap(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128(200u128),
            },
            Coin {
                denom: "ukrw".to_string(),
                amount: Uint128(300u128),
            }
        ]
    );
}

#[test]
fn supply_querier() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_token_balances(&[(
        &"liquidity0000".to_string(),
        &[
            (&MOCK_CONTRACT_ADDR.to_string(), &Uint128(123u128)),
            (&"addr00000".to_string(), &Uint128(123u128)),
            (&"addr00001".to_string(), &Uint128(123u128)),
            (&"addr00002".to_string(), &Uint128(123u128)),
        ],
    )]);

    assert_eq!(
        query_supply(&deps.as_ref().querier, Addr::unchecked("liquidity0000")).unwrap(),
        Uint128(492u128)
    )
}

#[test]
fn test_asset_info() {
    let token_info: AssetInfo = AssetInfo::Cw20(Addr::unchecked("asset0000"));
    let native_token_info: AssetInfo = AssetInfo::Native("uusd".to_string());

    assert_eq!(true, native_token_info.is_native_token());
    assert_eq!(false, token_info.is_native_token());

    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128(123),
    }]);
    deps.querier.with_token_balances(&[(
        &"asset0000".to_string(),
        &[
            (&MOCK_CONTRACT_ADDR.to_string(), &Uint128(123u128)),
            (&"addr00000".to_string(), &Uint128(123u128)),
            (&"addr00001".to_string(), &Uint128(123u128)),
            (&"addr00002".to_string(), &Uint128(123u128)),
        ],
    )]);

    // TODO: expose pool queries

    // assert_eq!(
    //     token_info
    //         .query_pool(&deps.as_ref().querier, Addr::unchecked(MOCK_CONTRACT_ADDR))
    //         .unwrap(),
    //     Uint128(123u128)
    // );
    // assert_eq!(
    //     native_token_info
    //         .query_pool(&deps.as_ref().querier, Addr::unchecked(MOCK_CONTRACT_ADDR))
    //         .unwrap(),
    //     Uint128(123u128)
    // );
}

#[test]
fn test_asset() {
    let mut deps = mock_dependencies(&[Coin {
        denom: "uusd".to_string(),
        amount: Uint128(123),
    }]);

    deps.querier.with_token_balances(&[(
        &"asset0000".to_string(),
        &[
            (&MOCK_CONTRACT_ADDR.to_string(), &Uint128(123u128)),
            (&"addr00000".to_string(), &Uint128(123u128)),
            (&"addr00001".to_string(), &Uint128(123u128)),
            (&"addr00002".to_string(), &Uint128(123u128)),
        ],
    )]);

    let token_asset = Asset::Cw20(Cw20CoinVerified {
        amount: Uint128(123123u128),
        address: Addr::unchecked("asset0000"),
    });
    let native_token_asset = Asset::from(coins(121903u128, "uusd"));

    assert_eq!(
        send_asset(token_asset, Addr::unchecked("rcpt")).unwrap(),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "asset0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: "rcpt".to_string(),
                amount: Uint128(123123u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    assert_eq!(
        send_asset(native_token_asset, Addr::unchecked("rcpt")).unwrap(),
        CosmosMsg::Bank(BankMsg::Send {
            to_address: "rcpt".to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128(121903u128),
            }]
        })
    );
}

#[test]
fn query_tfi_pair_contract() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_tfi_pairs(&[(
        &"asset0000uusd".to_string(),
        &PairInfo {
            asset_infos: [
                AssetInfo::Cw20(Addr::unchecked("asset0000")),
                AssetInfo::Native("uusd".to_string()),
            ],
            contract_addr: Addr::unchecked("pair0000"),
            liquidity_token: Addr::unchecked("liquidity0000"),
        },
    )]);

    let pair_info: PairInfo = query_pair_info(
        &deps.as_ref().querier,
        Addr::unchecked(MOCK_CONTRACT_ADDR),
        &[
            AssetInfo::Cw20(Addr::unchecked("asset0000")),
            AssetInfo::Native("uusd".to_string()),
        ],
    )
    .unwrap();

    assert_eq!(pair_info.contract_addr, Addr::unchecked("pair0000"),);
    assert_eq!(pair_info.liquidity_token, Addr::unchecked("liquidity0000"),);
}
