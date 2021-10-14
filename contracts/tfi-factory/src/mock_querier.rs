use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_slice, to_binary, Coin, ContractResult, Empty, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult, WasmQuery,
};
use std::{collections::HashMap, marker::PhantomData};
use tfi::asset::{AssetInfo, PairInfo};

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        api: MockApi::default(),
        storage: MockStorage::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
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
                })
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
                            })
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
                    panic!("DO NOT ENTER HERE")
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            tfi_pair_querier: TfiPairQuerier::default(),
        }
    }

    // configure the tfi pair
    pub fn with_tfi_pairs(&mut self, pairs: &[(&String, &PairInfo)]) {
        self.tfi_pair_querier = TfiPairQuerier::new(pairs);
    }

    // pub fn with_balance(&mut self, balances: &[(&HumanAddr, &[Coin])]) {
    //     for (addr, balance) in balances {
    //         self.base.update_balance(addr, balance.to_vec());
    //     }
    // }
}
