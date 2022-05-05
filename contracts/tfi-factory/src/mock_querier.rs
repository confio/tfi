use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, marker::PhantomData};

use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Addr, Coin, ContractResult, Empty, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult, WasmQuery,
};

use tfi::asset::{AssetInfo, PairInfo};

// Used for the create pair test
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

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier = WasmMockQuerier::new(
        &Addr::unchecked(MOCK_CONTRACT_ADDR),
        MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]),
    );

    OwnedDeps {
        api: MockApi::default(),
        storage: MockStorage::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier,
    contract: String,
    tfi_pair_querier: TfiPairQuerier,
}

#[derive(Clone, Default)]
pub struct TfiPairQuerier {
    pairs: HashMap<String, PairInfo>,
}

impl TfiPairQuerier {
    pub fn new(pairs: &[(&String, &PairInfo)]) -> Self {
        TfiPairQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &PairInfo)]) -> HashMap<String, PairInfo> {
    let mut pairs_map: HashMap<String, PairInfo> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(key.to_string(), (*pair).clone());
    }
    pairs_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                });
            }
        };
        self.handle_query(&request)
    }
}

impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                let key: &[u8] = key.as_slice();
                let prefix_pair_info = b"pair_info".to_vec();

                if key.to_vec() == prefix_pair_info {
                    let pair_info: PairInfo = match self.tfi_pair_querier.pairs.get(contract_addr) {
                        Some(v) => v.clone(),
                        None => {
                            return SystemResult::Err(SystemError::InvalidRequest {
                                error: format!("PairInfo is not found for {}", contract_addr),
                                request: key.into(),
                            });
                        }
                    };

                    SystemResult::Ok(ContractResult::from(to_binary(
                        &PairInfo::new(
                            [
                                AssetInfo::Native("uusd".to_string()),
                                AssetInfo::Native("uusd".to_string()),
                            ],
                            pair_info.contract_addr.clone(),
                            pair_info.liquidity_token,
                        )
                        .with_commission(pair_info.commission),
                    )))
                } else {
                    SystemResult::Err(SystemError::UnsupportedRequest {
                        kind: "key not supported".to_string(),
                    })
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart { .. }) => {
                SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: "WasmQuery::Smart".to_string(),
                })
            }
            QueryRequest::Wasm(WasmQuery::ContractInfo { contract_addr }) => {
                self.query_contract_info(contract_addr)
            }
            _ => self.base.handle_query(request),
        }
    }

    fn query_contract_info(&self, contract_addr: &str) -> QuerierResult {
        if contract_addr != self.contract {
            SystemResult::Err(SystemError::NoSuchContract {
                addr: contract_addr.into(),
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

impl WasmMockQuerier {
    pub fn new(contract: &Addr, base: MockQuerier) -> Self {
        WasmMockQuerier {
            base,
            contract: contract.to_string(),
            tfi_pair_querier: TfiPairQuerier::default(),
        }
    }

    // configure the tfi pair
    pub fn with_tfi_pairs(&mut self, pairs: &[(&String, &PairInfo)]) {
        self.tfi_pair_querier = TfiPairQuerier::new(pairs);
    }
}
