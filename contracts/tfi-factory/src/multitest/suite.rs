use anyhow::{anyhow, Result};
use cosmwasm_std::{coins, to_binary, Addr, BankMsg, Decimal, Empty, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg};
use cw4::{Cw4Contract, Member};
use cw4_group::msg::ExecuteMsg as Cw4ExecuteMsg;
use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};
use derivative::Derivative;
use tfi::asset::{Asset, AssetInfo, PairInfo};
use tfi::factory::{ExecuteMsg, InstantiateMsg, QueryMsg};
use tfi::pair::{Cw20HookMsg, ExecuteMsg as PairExecuteMsg};

const FEDERAL_RESERVE: &str = "reserve";
const DENOM: &str = "btc";

fn mock_app() -> App {
    AppBuilder::new_custom().build(|router, _, storage| {
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked(FEDERAL_RESERVE),
                coins(100000, DENOM),
            )
            .unwrap();
    })
}

fn contract_factory() -> Box<dyn Contract<Empty>> {
    Box::new(
        ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        )
        .with_reply(crate::contract::reply),
    )
}

fn contract_pair() -> Box<dyn Contract<Empty>> {
    Box::new(
        ContractWrapper::new(
            tfi_pair::contract::execute,
            tfi_pair::contract::instantiate,
            tfi_pair::contract::query,
        )
        .with_reply(tfi_pair::contract::reply),
    )
}

fn contract_cw20() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ))
}

fn contract_token() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        dso_token::contract::execute,
        dso_token::contract::instantiate,
        dso_token::contract::query,
    ))
}

fn contract_group() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        cw4_group::contract::execute,
        cw4_group::contract::instantiate,
        cw4_group::contract::query,
    ))
}

/// Testing environment with:
/// * single native token "btc"
/// * single cw4-group used as whitelist
/// * single dso-token "cash" using internal group
/// * single tfi-factory
/// * number of actors which are just address initialized with some "btc" and "cash"
///
/// Note, that only actors marked as whitelisted are initially on whitelist - none of owner,
/// whitelist, cash nor factory are initiallt on whitelist
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Suite {
    /// Application mock
    #[derivative(Debug = "ignore")]
    pub app: App,
    /// Special account for performing administrative executions
    pub owner: Addr,
    /// General purpose actors
    pub actors: Vec<Addr>,
    /// cw4 whitelist contract address
    pub whitelist: Cw4Contract,
    /// dso-token cash contract address
    pub cash: Cw20Contract,
    /// tfi-factory contract address
    pub factory: Addr,
}

impl Suite {
    /// Returns btc asset info
    ///
    /// Takes self only to have signature unified with `cash` so tests are more straightforward to
    /// read
    pub fn btc(&self) -> AssetInfo {
        AssetInfo::Native("btc".to_owned())
    }

    /// Returns cash asset info
    pub fn cash(&self) -> AssetInfo {
        AssetInfo::Token(self.cash.addr())
    }

    /// Executes CreatePair on `factory`. Returns created pair address and its liquidity token
    /// address.
    pub fn create_pair(
        &mut self,
        asset_infos: [AssetInfo; 2],
        commission: impl Into<Option<Decimal>>,
    ) -> Result<(Addr, Cw20Contract)> {
        self.app
            .execute_contract(
                self.owner.clone(),
                self.factory.clone(),
                &ExecuteMsg::CreatePair {
                    asset_infos: asset_infos.clone(),
                    commission: commission.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        let res: PairInfo = self
            .app
            .wrap()
            .query_wasm_smart(self.factory.clone(), &QueryMsg::Pair { asset_infos })?;

        Ok((res.contract_addr, Cw20Contract(res.liquidity_token)))
    }

    /// Adds member to whitelist
    pub fn add_member(&mut self, addr: &Addr) -> Result<&mut Self> {
        self.app
            .execute_contract(
                self.owner.clone(),
                self.whitelist.addr(),
                &Cw4ExecuteMsg::UpdateMembers {
                    add: vec![Member {
                        addr: addr.to_string(),
                        weight: 10,
                    }],
                    remove: vec![],
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes increase allowance on cw20 contract
    pub fn increase_allowance(
        &mut self,
        contract: &Addr,
        owner: &Addr,
        spender: &Addr,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                owner.clone(),
                contract.clone(),
                &Cw20ExecuteMsg::IncreaseAllowance {
                    spender: spender.to_string(),
                    amount: amount.into(),
                    expires: None,
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Provides liquidity on given pair
    pub fn provide_liquidity(
        &mut self,
        pair: &Addr,
        liquidity_provider: &Addr,
        btc: u128,
        cash: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                liquidity_provider.clone(),
                pair.clone(),
                &PairExecuteMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: self.btc(),
                            amount: btc.into(),
                        },
                        Asset {
                            info: self.cash(),
                            amount: cash.into(),
                        },
                    ],
                    slippage_tolerance: None,
                },
                &coins(btc, "btc"),
            )
            .map_err(|err| anyhow!(err))?;
        Ok(self)
    }

    /// Swaps btc for cash using given pair
    pub fn swap_btc(&mut self, pair: &Addr, trader: &Addr, btc: u128) -> Result<&mut Self> {
        self.app
            .execute_contract(
                trader.clone(),
                pair.clone(),
                &PairExecuteMsg::Swap {
                    offer_asset: Asset {
                        info: AssetInfo::Native("btc".to_owned()),
                        amount: Uint128::new(btc),
                    },
                    belief_price: None,
                    max_spread: None,
                    to: None,
                },
                &coins(btc, "btc"),
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Swap cash for btc using given pair
    pub fn swap_cash(&mut self, pair: &Addr, trader: &Addr, cash: u128) -> Result<&mut Self> {
        self.app
            .execute_contract(
                trader.clone(),
                self.cash.addr(),
                &cw20_base::msg::ExecuteMsg::Send {
                    contract: pair.to_string(),
                    amount: Uint128::new(cash),
                    msg: to_binary(&Cw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })
                    .unwrap(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Withdraws liquidity from given pair
    pub fn withdraw_liquidity(
        &mut self,
        pair: &Addr,
        lt: &Addr,
        lp: &Addr,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                lp.clone(),
                lt.clone(),
                &cw20_base::msg::ExecuteMsg::Send {
                    contract: pair.to_string(),
                    amount: amount.into(),
                    msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }
}

/// Configuration of single actor
struct ActorConfig {
    /// Actor address
    addr: String,
    /// Initial cash amount
    cash: u128,
    /// Initial btc amount
    btc: u128,
    /// Is actor initially whitelisted?
    whitelisted: bool,
}

/// Intermediate actor data
struct Actor {
    addr: Addr,
    whitelisted: bool,
}

#[derive(Default)]
pub struct Config {
    /// Initial actors
    actors: Vec<ActorConfig>,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_actor(
        mut self,
        addr: impl Into<String>,
        btc: u128,
        cash: u128,
        whitelisted: bool,
    ) -> Self {
        self.actors.push(ActorConfig {
            addr: addr.into(),
            btc,
            cash,
            whitelisted,
        });

        self
    }

    /// Initializes actors
    ///
    /// Sets initial btc balance, and returns:
    /// * actors data for further processing
    /// * data to be passed as initial cash balance
    ///
    /// Actors data are pairs of:
    /// * actor address
    /// * flag if actor should be initiallt whitelisted
    fn init_actors(actors: Vec<ActorConfig>, app: &mut App) -> Result<(Vec<Actor>, Vec<Cw20Coin>)> {
        Ok(actors
            .into_iter()
            .map(|actor| -> Result<_> {
                let addr = Addr::unchecked(&actor.addr);
                app.execute(
                    Addr::unchecked(FEDERAL_RESERVE),
                    BankMsg::Send {
                        to_address: actor.addr.to_owned(),
                        amount: coins(actor.btc, DENOM),
                    }
                    .into(),
                )
                .unwrap();

                let initial_cash = Cw20Coin {
                    address: addr.to_string(),
                    amount: Uint128::new(actor.cash),
                };

                Ok((
                    Actor {
                        addr,
                        whitelisted: actor.whitelisted,
                    },
                    initial_cash,
                ))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip())
    }

    /// Initializes whitelist contract basing on initial members
    pub fn init_whitelist(
        members: impl Iterator<Item = Addr>,
        app: &mut App,
        owner: &Addr,
        cw4_id: u64,
    ) -> Result<Cw4Contract> {
        let members = members
            .map(|addr| Member {
                addr: addr.to_string(),
                weight: 10,
            })
            .collect();

        app.instantiate_contract(
            cw4_id,
            owner.clone(),
            &cw4_group::msg::InstantiateMsg {
                admin: Some(owner.to_string()),
                members,
            },
            &[],
            "Whitelist",
            None,
        )
        .map(Cw4Contract)
        .map_err(|err| anyhow!(err))
    }

    /// Initializes cash contract
    fn init_cash(
        initial_balances: Vec<Cw20Coin>,
        whitelist: &Addr,
        app: &mut App,
        owner: &Addr,
        cw20_id: u64,
    ) -> Result<Cw20Contract> {
        app.instantiate_contract(
            cw20_id,
            owner.clone(),
            &dso_token::msg::InstantiateMsg {
                name: "Cash Token".to_owned(),
                symbol: "CASH".to_owned(),
                decimals: 9,
                initial_balances,
                mint: None,
                marketing: None,
                whitelist_group: whitelist.to_string(),
            },
            &[],
            "Cash",
            None,
        )
        .map(Cw20Contract)
        .map_err(|err| anyhow!(err))
    }

    /// Initializes factory contract
    fn init_factory(
        pair_id: u64,
        cw20_id: u64,
        app: &mut App,
        owner: &Addr,
        factory_id: u64,
    ) -> Result<Addr> {
        app.instantiate_contract(
            factory_id,
            owner.clone(),
            &InstantiateMsg::new(pair_id, cw20_id),
            &[],
            "Factory",
            None,
        )
        .map_err(|err| anyhow!(err))
    }

    pub fn init(self) -> Result<Suite> {
        let mut app = mock_app();
        let owner = Addr::unchecked("owner");
        let cw4_id = app.store_code(contract_group());
        let cw20_id = app.store_code(contract_cw20());
        let token_id = app.store_code(contract_token());
        let pair_id = app.store_code(contract_pair());
        let factory_id = app.store_code(contract_factory());

        let (actors, initial_cash) = Self::init_actors(self.actors, &mut app)?;
        let members = actors
            .iter()
            .filter(|actor| actor.whitelisted)
            .map(|actor| actor.addr.clone());
        let whitelist = Self::init_whitelist(members, &mut app, &owner, cw4_id)?;
        let actors = actors.into_iter().map(|actor| actor.addr).collect();
        let cash = Self::init_cash(initial_cash, &whitelist.addr(), &mut app, &owner, token_id)?;
        let factory = Self::init_factory(pair_id, cw20_id, &mut app, &owner, factory_id)?;

        Ok(Suite {
            app,
            owner,
            actors,
            whitelist,
            cash,
            factory,
        })
    }
}
