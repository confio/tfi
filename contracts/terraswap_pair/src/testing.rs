use crate::contract::{
    handle, init, query_config_asset, query_config_general, query_config_swap, query_pool,
    query_reverse_simulation, query_simulation,
};
use crate::math::{decimal_multiplication, reverse_decimal};
use crate::mock_querier::mock_dependencies;
use crate::msg::{
    ConfigAssetResponse, ConfigGeneralResponse, ConfigSwapResponse, Cw20HookMsg, HandleMsg,
    PoolResponse, ReverseSimulationResponse, SimulationResponse,
};
use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    log, to_binary, BankMsg, BlockInfo, Coin, CosmosMsg, Decimal, Env, HandleResponse, HumanAddr,
    StdError, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg, MinterResponse};
use terraswap::{Asset, AssetInfo, InitHook, PairInitMsg, TokenInitMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let mut msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(1),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: Some(InitHook {
            contract_addr: HumanAddr::from("factory0000"),
            msg: to_binary(&Uint128(1000000u128)).unwrap(),
        }),
    };

    let env = mock_env("addr0000", &[]);
    let res = init(&mut deps, env, msg.clone()).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => {
            assert_eq!(msg, "LP commission cannot be smaller than 0.25%")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // we can just call .unwrap() to assert this was a success
    msg.lp_commission = Decimal::permille(3);
    let env = mock_env("addr0000", &[]);
    let res = init(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id: 10u64,
                msg: to_binary(&TokenInitMsg {
                    name: "terraswap liquidity token".to_string(),
                    symbol: "uLP".to_string(),
                    decimals: 6,
                    initial_balances: vec![],
                    mint: Some(MinterResponse {
                        minter: HumanAddr::from(MOCK_CONTRACT_ADDR),
                        cap: None,
                    }),
                    init_hook: Some(InitHook {
                        msg: to_binary(&HandleMsg::PostInitialize {}).unwrap(),
                        contract_addr: HumanAddr::from(MOCK_CONTRACT_ADDR),
                    }),
                })
                .unwrap(),
                send: vec![],
                label: None,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("factory0000"),
                msg: to_binary(&Uint128(1000000u128)).unwrap(),
                send: vec![],
            })
        ]
    );

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // cannot change it after post intialization
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0001", &[]);
    let _res = handle(&mut deps, env, msg).unwrap_err();

    // // it worked, let's query the state
    let config_general: ConfigGeneralResponse = query_config_general(&deps).unwrap();
    assert_eq!("addr0000", config_general.owner.as_str());
    assert_eq!(
        "collector0000",
        config_general.commission_collector.as_str()
    );
    assert_eq!("liquidity0000", config_general.liquidity_token.as_str());

    let config_asset: ConfigAssetResponse = query_config_asset(&deps).unwrap();
    assert_eq!(
        config_asset.infos,
        [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000")
            }
        ]
    );

    let config_swap: ConfigSwapResponse = query_config_swap(&deps).unwrap();
    assert_eq!(Decimal::permille(3), config_swap.lp_commission);
    assert_eq!(Decimal::permille(1), config_swap.owner_commission);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);
    let msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(3),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: None,
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // update owner
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: Some(HumanAddr("addr0001".to_string())),
        lp_commission: None,
        owner_commission: None,
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let config_general: ConfigGeneralResponse = query_config_general(&deps).unwrap();
    assert_eq!("addr0001", config_general.owner.as_str());

    // update left items
    let env = mock_env("addr0001", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        lp_commission: Some(Decimal::percent(1)),
        owner_commission: Some(Decimal::percent(2)),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let config_swap: ConfigSwapResponse = query_config_swap(&deps).unwrap();
    assert_eq!(Decimal::percent(1), config_swap.lp_commission);
    assert_eq!(Decimal::percent(2), config_swap.owner_commission);

    // Unauthorzied err
    let env = mock_env("addr0000", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner: None,
        lp_commission: None,
        owner_commission: None,
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn provide_liquidity() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(200u128),
        }],
    );

    deps.querier.with_token_balances(&[(
        &HumanAddr::from("liquidity0000"),
        &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(0))],
    )]);

    let msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(3),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: None,
    };

    let env = mock_env("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // successfully provide liquidity for the exist pool
    let msg = HandleMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000".to_string()),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(100u128),
            },
        ],
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
    );
    let res = handle(&mut deps, env, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );
    assert_eq!(
        mint_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("liquidity0000"),
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    // provide more liquidity 1:2, which is not propotional to 1:1,
    // then it must accept 1:1 and treat left amount as donation
    deps.querier.with_balance(&[(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(200 + 200 /* user deposit must be pre-applied */),
        }],
    )]);

    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100))],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(200))],
        ),
    ]);

    let msg = HandleMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000".to_string()),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(200u128),
            },
        ],
    };

    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(200u128),
        }],
        1000,
    );

    // only accept 100, then 50 share will be generated with 100 * (100 / 200)
    let res: HandleResponse = handle(&mut deps, env, msg).unwrap();
    let transfer_from_msg = res.messages.get(0).expect("no message");
    let mint_msg = res.messages.get(1).expect("no message");
    assert_eq!(
        transfer_from_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: HumanAddr::from("addr0000"),
                recipient: HumanAddr::from(MOCK_CONTRACT_ADDR),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );
    assert_eq!(
        mint_msg,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("liquidity0000"),
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(50u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    // current liquidity is 2:3; lets put more to make it 1:1
    // then no liquidity tokens will be issued
    let msg = HandleMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000".to_string()),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(0u128),
            },
        ],
    };

    let env = mock_env_with_block_time("addr0001", &[], 1000);
    let _res = handle(&mut deps, env, msg).unwrap();

    // check wrong argument
    let msg = HandleMsg::ProvideLiquidity {
        assets: [
            Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000".to_string()),
                },
                amount: Uint128::from(100u128),
            },
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: Uint128::from(50u128),
            },
        ],
    };

    let env = mock_env(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
    );
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::GenericErr { msg, .. } => assert_eq!(
            msg,
            "Native token balance missmatch between the argument and the transferred".to_string()
        ),
        _ => panic!("Must return generic error"),
    }
}

#[test]
fn withdraw_liquidity() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128(100u128),
        }],
    );

    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from("addr0000"), &Uint128(100u128))],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &Uint128(100u128))],
        ),
    ]);

    let msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(3),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: None,
    };

    let env = mock_env("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // withdraw liquidity
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        msg: Some(to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap()),
        amount: Uint128(100u128),
    });

    let env = mock_env("liquidity0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    let log_withdrawn_share = res.log.get(1).expect("no log");
    let log_refund_assets = res.log.get(2).expect("no log");
    let msg_refund_0 = res.messages.get(0).expect("no message");
    let msg_refund_1 = res.messages.get(1).expect("no message");
    let msg_burn_liquidity = res.messages.get(2).expect("no message");
    assert_eq!(
        msg_refund_0,
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100u128),
            }],
        })
    );
    assert_eq!(
        msg_refund_1,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );
    assert_eq!(
        msg_burn_liquidity,
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("liquidity0000"),
            msg: to_binary(&Cw20HandleMsg::Burn {
                amount: Uint128::from(100u128),
            })
            .unwrap(),
            send: vec![],
        })
    );

    assert_eq!(
        log_withdrawn_share,
        &log("withdrawn_share", 100u128.to_string())
    );
    assert_eq!(
        log_refund_assets,
        &log("refund_assets", "100uusd, 100asset0000")
    );
}

#[test]
fn try_native_to_token() {
    let total_share = Uint128(30000000000u128);
    let asset_pool_amount = Uint128(20000000000u128);
    let collateral_pool_amount = Uint128(30000000000u128);
    let price = Decimal::from_ratio(collateral_pool_amount, asset_pool_amount);
    let exchange_rate = reverse_decimal(price);
    let offer_amount = Uint128(1500000000u128);

    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount + offer_amount, /* user deposit must be pre-applied */
        }],
    );

    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );

    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &asset_pool_amount)],
        ),
    ]);

    let msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(3),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: None,
    };

    let env = mock_env("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // normal swap
    let msg = HandleMsg::Swap {
        offer_asset: Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: offer_amount,
        },
        max_spread: None,
    };
    let env = mock_env_with_block_time(
        "addr0000",
        &[Coin {
            denom: "uusd".to_string(),
            amount: offer_amount,
        }],
        1000,
    );

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");
    let msg_commission_transfer = res.messages.get(1).expect("no message");

    // current price is 1.5, so expected return without spread is 1000
    // 952.380953 = 20000 - 20000 * 30000 / (30000 + 1500)
    let expected_ret_amount = Uint128(952_380_953u128);
    let expected_spread_amount = (offer_amount * exchange_rate - expected_ret_amount).unwrap();
    let expected_lp_commission = expected_ret_amount.multiply_ratio(3u128, 1000u128); // 0.2%
    let expected_owner_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_lp_commission + expected_owner_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();
    let expected_tax_amount = Uint128::zero(); // no tax for token

    // check simulation res
    deps.querier.with_balance(&[(
        &HumanAddr::from(MOCK_CONTRACT_ADDR),
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount, /* user deposit must be pre-applied */
        }],
    )]);

    let simulation_res: SimulationResponse = query_simulation(
        &deps,
        Asset {
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            amount: offer_amount,
        },
    )
    .unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount);
    assert_eq!(expected_commission_amount, simulation_res.commission_amount);
    assert_eq!(expected_spread_amount, simulation_res.spread_amount);

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse = query_reverse_simulation(
        &deps,
        Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: expected_return_amount,
        },
    )
    .unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.u128() as i128).abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_spread_amount.u128() as i128
            - reverse_simulation_res.spread_amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );

    assert_eq!(
        res.log,
        vec![
            log("action", "swap"),
            log("offer_asset", "uusd"),
            log("ask_asset", "asset0000"),
            log("offer_amount", offer_amount.to_string()),
            log("return_amount", expected_return_amount.to_string()),
            log("tax_amount", expected_tax_amount.to_string()),
            log("spread_amount", expected_spread_amount.to_string()),
            log("commission_amount", expected_commission_amount.to_string()),
        ]
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("addr0000"),
                amount: Uint128::from(expected_return_amount),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("asset0000"),
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: HumanAddr::from("collector0000"),
                amount: Uint128::from(expected_owner_commission),
            })
            .unwrap(),
            send: vec![],
        }),
        msg_commission_transfer,
    );
}

#[test]
fn try_token_to_native() {
    let total_share = Uint128(20000000000u128);
    let asset_pool_amount = Uint128(30000000000u128);
    let collateral_pool_amount = Uint128(20000000000u128);
    let price = Decimal::from_ratio(collateral_pool_amount, asset_pool_amount);
    let exchange_rate = decimal_multiplication(price, Decimal::one());
    let offer_amount = Uint128(1500000000u128);

    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: collateral_pool_amount,
        }],
    );
    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );
    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(
                &HumanAddr::from(MOCK_CONTRACT_ADDR),
                &(asset_pool_amount + offer_amount),
            )],
        ),
    ]);

    let msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(3),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: None,
    };

    let env = mock_env("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    // unauthorized access; can not execute swap directy for token swap
    let msg = HandleMsg::Swap {
        offer_asset: Asset {
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
            amount: offer_amount,
        },
        max_spread: None,
    };
    let env = mock_env_with_block_time("addr0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => (),
        _ => panic!("DO NOT ENTER HERE"),
    }

    // normal sell
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: offer_amount,
        msg: Some(to_binary(&Cw20HookMsg::Swap { max_spread: None }).unwrap()),
    });
    let env = mock_env_with_block_time("asset0000", &[], 1000);

    let res = handle(&mut deps, env, msg).unwrap();
    let msg_transfer = res.messages.get(0).expect("no message");
    let msg_commission_transfer = res.messages.get(1).expect("no message");

    // current price is 1.5, so expected return without spread is 1000
    // 952.380953 = 20000 - 20000 * 30000 / (30000 + 1500)
    let expected_ret_amount = Uint128(952_380_953u128);
    let expected_spread_amount = (offer_amount * exchange_rate - expected_ret_amount).unwrap();
    let expected_lp_commission = expected_ret_amount.multiply_ratio(3u128, 1000u128); // 0.3%
    let expected_owner_commission = expected_ret_amount.multiply_ratio(1u128, 1000u128); // 0.1%
    let expected_commission_amount = expected_lp_commission + expected_owner_commission;
    let expected_return_amount = (expected_ret_amount - expected_commission_amount).unwrap();
    let expected_tax_amount = std::cmp::min(
        Uint128(1000000u128),
        (expected_return_amount
            - expected_return_amount.multiply_ratio(Uint128(100u128), Uint128(101u128)))
        .unwrap(),
    );
    let expected_owner_commission_tax_amount = std::cmp::min(
        Uint128(1000000u128),
        (expected_owner_commission
            - expected_owner_commission.multiply_ratio(Uint128(100u128), Uint128(101u128)))
        .unwrap(),
    );

    // check simulation res
    // return asset token balance as normal
    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share)],
        ),
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &(asset_pool_amount))],
        ),
    ]);

    let simulation_res: SimulationResponse = query_simulation(
        &deps,
        Asset {
            amount: offer_amount,
            info: AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        },
    )
    .unwrap();
    assert_eq!(expected_return_amount, simulation_res.return_amount);
    assert_eq!(expected_commission_amount, simulation_res.commission_amount);
    assert_eq!(expected_spread_amount, simulation_res.spread_amount);

    // check reverse simulation res
    let reverse_simulation_res: ReverseSimulationResponse = query_reverse_simulation(
        &deps,
        Asset {
            amount: expected_return_amount,
            info: AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        },
    )
    .unwrap();
    assert_eq!(
        (offer_amount.u128() as i128 - reverse_simulation_res.offer_amount.u128() as i128).abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_commission_amount.u128() as i128
            - reverse_simulation_res.commission_amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );
    assert_eq!(
        (expected_spread_amount.u128() as i128
            - reverse_simulation_res.spread_amount.u128() as i128)
            .abs()
            < 5i128,
        true
    );

    assert_eq!(
        res.log,
        vec![
            log("action", "swap"),
            log("offer_asset", "asset0000"),
            log("ask_asset", "uusd"),
            log("offer_amount", offer_amount.to_string()),
            log("return_amount", expected_return_amount.to_string()),
            log("tax_amount", expected_tax_amount.to_string()),
            log("spread_amount", expected_spread_amount.to_string()),
            log("commission_amount", expected_commission_amount.to_string()),
        ]
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("addr0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: (expected_return_amount - expected_tax_amount).unwrap(),
            }],
        }),
        msg_transfer,
    );

    assert_eq!(
        &CosmosMsg::Bank(BankMsg::Send {
            from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
            to_address: HumanAddr::from("collector0000"),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: (expected_owner_commission - expected_owner_commission_tax_amount).unwrap(),
            }],
        }),
        msg_commission_transfer,
    );

    // failed due to non asset token contract try to execute sell
    let msg = HandleMsg::Receive(Cw20ReceiveMsg {
        sender: HumanAddr::from("addr0000"),
        amount: offer_amount,
        msg: Some(to_binary(&Cw20HookMsg::Swap { max_spread: None }).unwrap()),
    });
    let env = mock_env_with_block_time("liquidtity0000", &[], 1000);
    let res = handle(&mut deps, env, msg).unwrap_err();
    match res {
        StdError::Unauthorized { .. } => (),
        _ => panic!("DO NOT ENTER HERE"),
    }
}

#[test]
fn test_deduct() {
    let mut deps = mock_dependencies(20, &[]);

    let tax_rate = Decimal::percent(2);
    let tax_cap = Uint128::from(1_000_000u128);
    deps.querier.with_tax(
        Decimal::percent(2),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );

    let amount = Uint128(1000_000_000u128);
    let expected_after_amount = std::cmp::max(
        (amount - amount * tax_rate).unwrap(),
        (amount - tax_cap).unwrap(),
    );

    let after_amount = (Asset {
        info: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        amount,
    })
    .deduct_tax(&deps)
    .unwrap();

    assert_eq!(expected_after_amount, after_amount.amount);
}

#[test]
fn test_query_pool() {
    let total_share_amount = Uint128::from(111u128);
    let asset_0_amount = Uint128::from(222u128);
    let asset_1_amount = Uint128::from(333u128);
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: asset_0_amount,
        }],
    );

    deps.querier.with_token_balances(&[
        (
            &HumanAddr::from("asset0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &asset_1_amount)],
        ),
        (
            &HumanAddr::from("liquidity0000"),
            &[(&HumanAddr::from(MOCK_CONTRACT_ADDR), &total_share_amount)],
        ),
    ]);

    let msg = PairInitMsg {
        owner: HumanAddr("addr0000".to_string()),
        commission_collector: HumanAddr("collector0000".to_string()),
        lp_commission: Decimal::permille(3),
        owner_commission: Decimal::permille(1),
        asset_infos: [
            AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            AssetInfo::Token {
                contract_addr: HumanAddr::from("asset0000"),
            },
        ],
        token_code_id: 10u64,
        init_hook: None,
    };

    let env = mock_env("addr0000", &[]);
    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // post initalize
    let msg = HandleMsg::PostInitialize {};
    let env = mock_env("liquidity0000", &[]);
    let _res = handle(&mut deps, env, msg).unwrap();

    let res: PoolResponse = query_pool(&deps).unwrap();

    assert_eq!(
        res.assets,
        [
            Asset {
                info: AssetInfo::NativeToken {
                    denom: "uusd".to_string(),
                },
                amount: asset_0_amount
            },
            Asset {
                info: AssetInfo::Token {
                    contract_addr: HumanAddr::from("asset0000"),
                },
                amount: asset_1_amount
            }
        ]
    );
    assert_eq!(res.total_share, total_share_amount);
}

fn mock_env_with_block_time<U: Into<HumanAddr>>(sender: U, sent: &[Coin], time: u64) -> Env {
    let env = mock_env(sender, sent);
    // register time
    return Env {
        block: BlockInfo {
            height: 1,
            time,
            chain_id: "columbus".to_string(),
        },
        ..env
    };
}
