use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage};
use cosmwasm_std::{
    attr, from_binary, from_slice, to_binary, Addr, Binary, ContractResult, Decimal, Empty,
    OwnedDeps, Querier, QuerierResult, QueryRequest, Reply, ReplyOn, StdError, Storage, SubMsg,
    SubMsgExecutionResponse, SubMsgResult, SystemError, SystemResult, WasmMsg, WasmQuery,
};

use cw_storage_plus::Item;

use tfi::asset::{AssetInfo, PairInfo};
use tfi::factory::{ConfigResponse, ExecuteCreatePair, ExecuteMsg, InstantiateMsg, QueryMsg};
use tfi::pair::InstantiateMsg as PairInstantiateMsg;

use crate::contract::{execute, instantiate, query, reply};
use crate::error::ContractError;
use crate::mock_querier::mock_dependencies;
use crate::state::{pair_key, TmpPairInfo, TMP_PAIR_INFO};

// Used for the create pair test
pub const FACTORY_CONTRACT: Item<String> = Item::new("contract_info");
pub const FACTORY_ADDR: &str = "cosmos2contract";
pub const FACTORY_ADMIN: &str = "migrate_admin";

// Copied here because this struct is non-exhaustive.
// Needs new `new_with_admin()` helper
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ContractInfoResponse {
    pub code_id: u64,
    /// address that instantiated this contract
    pub creator: String,
    /// admin who can run migrations (if any)
    pub admin: Option<String>,
    /// if set, the contract is pinned to the cache, and thus uses less gas when called
    pub pinned: bool,
    /// set if this contract has bound an IBC port
    pub ibc_port: Option<String>,
}

struct FactoryQuerier {
    contract: String,
    storage: MockStorage,
}

impl FactoryQuerier {
    pub fn new(contract: &Addr, token_version: &str) -> Self {
        let mut storage = MockStorage::new();
        FACTORY_CONTRACT
            .save(&mut storage, &token_version.to_string())
            .unwrap();

        FactoryQuerier {
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
            QueryRequest::Wasm(WasmQuery::ContractInfo { contract_addr }) => {
                self.query_contract_info(contract_addr)
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

    fn query_contract_info(&self, contract_addr: String) -> QuerierResult {
        if contract_addr != self.contract {
            SystemResult::Err(SystemError::NoSuchContract {
                addr: contract_addr,
            })
        } else {
            let res = ContractInfoResponse {
                code_id: 1,
                creator: "creator".into(),
                admin: Some(FACTORY_ADMIN.into()),
                pinned: false,
                ibc_port: None,
            };
            let bin = to_binary(&res).unwrap();
            SystemResult::Ok(ContractResult::Ok(bin))
        }
    }
}

impl Querier for FactoryQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {:?}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(request)
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg::new(321u64, 123u64);

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let query_res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&query_res).unwrap();
    assert_eq!(123u64, config_res.token_code_id);
    assert_eq!(321u64, config_res.pair_code_id);
    assert_eq!("addr0000".to_string(), config_res.owner);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg::new(321u64, 123u64);

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // update owner
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("addr0001".to_string()),
        pair_code_id: None,
        token_code_id: None,
        default_commission: None,
    };

    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&query_res).unwrap();
    assert_eq!(123u64, config_res.token_code_id);
    assert_eq!(321u64, config_res.pair_code_id);
    assert_eq!("addr0001".to_string(), config_res.owner);
    assert_eq!(Decimal::permille(3), config_res.default_commission);

    // update ids
    let env = mock_env();
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        pair_code_id: Some(100u64),
        token_code_id: Some(200u64),
        default_commission: None,
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&query_res).unwrap();
    assert_eq!(200u64, config_res.token_code_id);
    assert_eq!(100u64, config_res.pair_code_id);
    assert_eq!("addr0001".to_string(), config_res.owner);
    assert_eq!(Decimal::permille(3), config_res.default_commission);

    // update default commission
    let env = mock_env();
    let info = mock_info("addr0001", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        pair_code_id: None,
        token_code_id: None,
        default_commission: Some(Decimal::permille(5)),
    };

    let res = execute(deps.as_mut(), env, info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let query_res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&query_res).unwrap();
    assert_eq!(200u64, config_res.token_code_id);
    assert_eq!(100u64, config_res.pair_code_id);
    assert_eq!("addr0001".to_string(), config_res.owner);
    assert_eq!(Decimal::permille(5), config_res.default_commission);

    // Unauthorized err
    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: None,
        pair_code_id: None,
        token_code_id: None,
        default_commission: None,
    };

    let res = execute(deps.as_mut(), env, info, msg);
    match res {
        Err(ContractError::Std(StdError::GenericErr { msg, .. })) => {
            assert_eq!(msg, "unauthorized")
        }
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn create_pair() {
    // Set and honour our local data (ContractInfo)
    let querier = FactoryQuerier::new(&Addr::unchecked(FACTORY_ADDR), "0.9");
    let mut deps = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: PhantomData,
    };

    let msg = InstantiateMsg::new(321u64, 123u64);

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), env, info, msg).unwrap();

    let asset_infos = [
        AssetInfo::Token(Addr::unchecked("asset0000")),
        AssetInfo::Token(Addr::unchecked("asset0001")),
    ];

    let env = mock_env();
    let info = mock_info("addr0000", &[]);
    let res = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteCreatePair::new(asset_infos.clone()).into(),
    )
    .unwrap();
    assert_eq!(
        res.attributes,
        vec![
            attr("action", "create_pair"),
            attr("pair", "asset0000-asset0001")
        ]
    );
    assert_eq!(
        res.messages,
        vec![SubMsg {
            id: 1,
            gas_limit: None,
            reply_on: ReplyOn::Success,
            msg: WasmMsg::Instantiate {
                msg: to_binary(&PairInstantiateMsg::new(asset_infos.clone(), 123u64)).unwrap(),
                code_id: 321u64,
                funds: vec![],
                label: "Tgrade finance trading pair".to_string(),
                admin: Some(FACTORY_ADMIN.into()),
            }
            .into()
        },]
    );

    assert_eq!(
        TMP_PAIR_INFO.load(&deps.storage).unwrap(),
        TmpPairInfo {
            asset_infos: asset_infos.clone(),
            pair_key: pair_key(&asset_infos),
            commission: Decimal::permille(3),
        }
    );
}

#[test]
fn reply_test() {
    let mut deps = mock_dependencies(&[]);

    let asset_infos = [
        AssetInfo::Token(Addr::unchecked("asset0000")),
        AssetInfo::Token(Addr::unchecked("asset0001")),
    ];

    let pair_key = pair_key(&asset_infos);
    TMP_PAIR_INFO
        .save(
            &mut deps.storage,
            &TmpPairInfo {
                asset_infos: asset_infos.clone(),
                pair_key,
                commission: Decimal::permille(3),
            },
        )
        .unwrap();

    let reply_msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(vec![10, 8, 112, 97, 105, 114, 48, 48, 48, 48].into()),
        }),
    };

    // register tfi pair querier
    deps.querier.with_tfi_pairs(&[(
        &"pair0000".to_string(),
        &PairInfo::new(
            [
                AssetInfo::Native("uusd".to_string()),
                AssetInfo::Native("uusd".to_string()),
            ],
            Addr::unchecked("pair0000"),
            Addr::unchecked("liquidity0000"),
        ),
    )]);

    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    let query_res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::Pair {
            asset_infos: asset_infos.clone(),
        },
    )
    .unwrap();

    let pair_res: PairInfo = from_binary(&query_res).unwrap();
    assert_eq!(
        pair_res,
        PairInfo::new(
            asset_infos,
            Addr::unchecked("pair0000"),
            Addr::unchecked("liquidity0000"),
        )
    );
}

#[test]
fn custom_default_commission() {
    // Set and honour our local data (ContractInfo)
    let querier = FactoryQuerier::new(&Addr::unchecked(FACTORY_ADDR), "0.9");
    let mut deps = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: PhantomData,
    };

    let msg = InstantiateMsg::new(321u64, 123u64).with_default_commission(Decimal::permille(5));

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), env, info, msg).unwrap();

    let asset_infos = [
        AssetInfo::Token(Addr::unchecked("asset0000")),
        AssetInfo::Token(Addr::unchecked("asset0001")),
    ];

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        ExecuteCreatePair::new(asset_infos.clone()).into(),
    )
    .unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "create_pair"),
            attr("pair", "asset0000-asset0001")
        ]
    );

    assert_eq!(
        res.messages,
        vec![SubMsg {
            id: 1,
            gas_limit: None,
            reply_on: ReplyOn::Success,
            msg: WasmMsg::Instantiate {
                msg: to_binary(
                    &PairInstantiateMsg::new(asset_infos.clone(), 123u64)
                        .with_commission(Decimal::permille(5))
                )
                .unwrap(),
                code_id: 321u64,
                funds: vec![],
                label: "Tgrade finance trading pair".to_string(),
                admin: Some(FACTORY_ADMIN.into()),
            }
            .into()
        },]
    );

    assert_eq!(
        TMP_PAIR_INFO.load(&deps.storage).unwrap(),
        TmpPairInfo {
            asset_infos: asset_infos.clone(),
            pair_key: pair_key(&asset_infos),
            commission: Decimal::permille(5),
        }
    );
}

#[test]
fn invalid_custom_default_commission() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg::new(321u64, 123u64).with_default_commission(Decimal::permille(1001));

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    let err = instantiate(deps.as_mut(), env, info, msg).unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidCommission(Decimal::permille(1001))
    );
}

#[test]
fn custom_pair_commission() {
    // Set and honour our local data (ContractInfo)
    let querier = FactoryQuerier::new(&Addr::unchecked(FACTORY_ADDR), "0.9");
    let mut deps = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: PhantomData,
    };

    let msg = InstantiateMsg::new(321u64, 123u64).with_default_commission(Decimal::permille(5));

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), env, info, msg).unwrap();

    let asset_infos = [
        AssetInfo::Token(Addr::unchecked("asset0000")),
        AssetInfo::Token(Addr::unchecked("asset0001")),
    ];

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        ExecuteCreatePair::new(asset_infos.clone())
            .with_commission(Decimal::permille(5))
            .into(),
    )
    .unwrap();

    assert_eq!(
        res.attributes,
        vec![
            attr("action", "create_pair"),
            attr("pair", "asset0000-asset0001")
        ]
    );

    assert_eq!(
        res.messages,
        vec![SubMsg {
            id: 1,
            gas_limit: None,
            reply_on: ReplyOn::Success,
            msg: WasmMsg::Instantiate {
                msg: to_binary(
                    &PairInstantiateMsg::new(asset_infos.clone(), 123u64)
                        .with_commission(Decimal::permille(5))
                )
                .unwrap(),
                code_id: 321u64,
                funds: vec![],
                label: "Tgrade finance trading pair".to_string(),
                admin: Some(FACTORY_ADMIN.into()),
            }
            .into()
        },]
    );

    assert_eq!(
        TMP_PAIR_INFO.load(&deps.storage).unwrap(),
        TmpPairInfo {
            asset_infos: asset_infos.clone(),
            pair_key: pair_key(&asset_infos),
            commission: Decimal::permille(5),
        }
    );
}

#[test]
fn invalid_custom_pair_commission() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg::new(321u64, 123u64).with_default_commission(Decimal::permille(5));

    let env = mock_env();
    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    instantiate(deps.as_mut(), env, info, msg).unwrap();

    let asset_infos = [
        AssetInfo::Token(Addr::unchecked("asset0000")),
        AssetInfo::Token(Addr::unchecked("asset0001")),
    ];

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("addr0000", &[]),
        ExecuteCreatePair::new(asset_infos)
            .with_commission(Decimal::permille(1001))
            .into(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        ContractError::InvalidCommission(Decimal::permille(1001))
    );
}
