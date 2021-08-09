use anyhow::{anyhow, Result};
use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{coin, coins, to_binary, Addr, Decimal, Empty, StdError, Uint128};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};
use derivative::Derivative;

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

/// Helper struct providing unified environment for tfi-pair testing
///
/// It assumes actors:
/// * btc: native token
/// * cash: cw20 token
/// * pair: tfi-pair contact between btc and cash
/// * lt: cw20 token, pair liquidity token
/// * traders: number of accounts performing swaps
/// * lp: numbers of actors providing liquidity
///
/// `traders` and `lp` doesn't differ between themself, the split is made only for better tests
/// readability
#[derive(Derivative)]
#[derivative(Debug)]
struct Suite {
    /// Multitest app
    #[derivative(Debug = "ignore")]
    app: App,
    /// Admin actor, so there is someone to perform test queries and executions
    admin: Addr,
    /// Cash cw20 contract address
    cash: Addr,
    /// Pair cw20 contract address
    pair: Addr,
    /// Pair liquidity token cw20 contract address
    lt: Addr,
    /// Traders addresses
    traders: Vec<Addr>,
    /// Liquidity providers adresses
    lps: Vec<Addr>,
}

impl Suite {
    /// Helper executing providing liquidity for pair
    ///
    /// First if any cash is provided, increases allowance for it so it can actually be send. Then
    /// `ProvideLiquidity` message is send to pair contract
    fn provide_liquidity(
        &mut self,
        lp: &Addr,
        btc: u128,
        cash: u128,
        slippage_tolerance: impl Into<Option<Decimal>>,
    ) -> Result<&mut Self> {
        if cash > 0 {
            self.app
                .execute_contract(
                    lp.clone(),
                    self.cash.clone(),
                    &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                        spender: self.pair.to_string(),
                        amount: Uint128::new(cash),
                        expires: None,
                    },
                    &[],
                )
                .map_err(|err| anyhow!(err))?;
        }

        self.app
            .execute_contract(
                lp.clone(),
                self.pair.clone(),
                &ExecuteMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: AssetInfo::Native("btc".to_owned()),
                            amount: Uint128::new(btc),
                        },
                        Asset {
                            info: AssetInfo::Token(self.cash.clone()),
                            amount: Uint128::new(cash),
                        },
                    ],
                    slippage_tolerance: slippage_tolerance.into(),
                },
                &coins(btc, "btc"),
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Helper swapping btc for cash on pair
    ///
    /// Executes `Swap` message on pair
    fn swap_btc(
        &mut self,
        trader: &Addr,
        btc: u128,
        belief_price: impl Into<Option<Decimal>>,
        max_spread: impl Into<Option<Decimal>>,
        to: impl Into<Option<Addr>>,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                trader.clone(),
                self.pair.clone(),
                &ExecuteMsg::Swap {
                    offer_asset: Asset {
                        info: AssetInfo::Native("btc".to_owned()),
                        amount: Uint128::new(btc),
                    },
                    belief_price: belief_price.into(),
                    max_spread: max_spread.into(),
                    to: to.into().as_ref().map(ToString::to_string),
                },
                &coins(btc, "btc"),
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Helper swapping cash for btc on pair
    ///
    /// Executes `Send` message on cash contract, with `Cw20HookMsg::Swap` message as hook
    fn swap_cash(
        &mut self,
        trader: &Addr,
        cash: u128,
        belief_price: impl Into<Option<Decimal>>,
        max_spread: impl Into<Option<Decimal>>,
        to: impl Into<Option<Addr>>,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                trader.clone(),
                self.cash.clone(),
                &cw20_base::msg::ExecuteMsg::Send {
                    contract: self.pair.to_string(),
                    amount: Uint128::new(cash),
                    msg: to_binary(&Cw20HookMsg::Swap {
                        belief_price: belief_price.into(),
                        max_spread: max_spread.into(),
                        to: to.into().as_ref().map(ToString::to_string),
                    })
                    .unwrap(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Helper for withdrawing liquidity from pair
    ///
    /// Executes `Send` on lt contract with `Cw20HookMsg::WithdrawLiquidity` as send hook message
    fn withdraw_liquidity(&mut self, lp: &Addr, lt: u128) -> Result<&mut Self> {
        self.app
            .execute_contract(
                lp.clone(),
                self.lt.clone(),
                &cw20_base::msg::ExecuteMsg::Send {
                    contract: self.pair.to_string(),
                    amount: Uint128::new(lt),
                    msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {}).unwrap(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Asserts if balances on account are as expected
    #[track_caller]
    fn assert_balances(&mut self, addr: &Addr, btc: u128, cash: u128, lt: u128) -> &mut Self {
        let btc_balance = self
            .app
            .wrap()
            .query_balance(addr.clone(), "btc")
            .unwrap_or_else(|_| panic!("Query for balance of btc for {} failed", addr));

        assert_eq!(
            btc_balance,
            coin(btc, "btc"),
            "Btc balace missmatch, expected: {}, actual: {}",
            btc,
            btc_balance.amount
        );

        let cash_balance: cw20::BalanceResponse = self
            .app
            .wrap()
            .query_wasm_smart(
                self.cash.clone(),
                &cw20_base::msg::QueryMsg::Balance {
                    address: addr.to_string(),
                },
            )
            .unwrap_or_else(|_| panic!("Query for balance of cash on {} failed", addr));

        assert_eq!(
            cash_balance,
            cw20::BalanceResponse {
                balance: Uint128::new(cash)
            },
            "Cash balance missmatch, expected: {}, actual: {}",
            cash,
            cash_balance.balance
        );

        let lt_balance: cw20::BalanceResponse = self
            .app
            .wrap()
            .query_wasm_smart(
                self.lt.clone(),
                &cw20_base::msg::QueryMsg::Balance {
                    address: addr.to_string(),
                },
            )
            .unwrap_or_else(|_| panic!("Query for balance of liquidity tokens on {} failed", addr));

        assert_eq!(
            lt_balance,
            cw20::BalanceResponse {
                balance: Uint128::new(lt)
            },
            "Liquidity tokens balance missmatch, expected: {}, actual: {}",
            lt,
            lt_balance.balance
        );

        self
    }
}

/// Initial trader/luqudity provider state
#[derive(Debug)]
struct ActorConfig {
    /// Actor addr
    addr: Addr,
    /// Actor initial btc balance
    btc: u128,
    /// Actor initial cash balance
    cash: u128,
}

/// Builder helping construction of `Suite` helper
#[derive(Debug, Default)]
struct SuiteConfig {
    /// Initial traders config
    traders: Vec<ActorConfig>,
    /// Initial liquidity providers config
    lps: Vec<ActorConfig>,
}

impl SuiteConfig {
    /// Creates new config without extra actors
    fn new() -> Self {
        Self::default()
    }

    /// Adds new traider to test suite
    fn with_trader(mut self, addr: &str, btc: u128, cash: u128) -> Self {
        self.traders.push(ActorConfig {
            addr: Addr::unchecked(addr),
            btc,
            cash,
        });

        self
    }

    /// Adds new liquidity provider to test suite
    fn with_liquidity_provider(mut self, addr: &str, btc: u128, cash: u128) -> Self {
        self.lps.push(ActorConfig {
            addr: Addr::unchecked(addr),
            btc,
            cash,
        });

        self
    }

    /// Initializes given actors with initial btc balance, returning back actors addresses and
    /// configuration of initial cash balance to be set later while creating cash contract
    fn init_actors(app: &mut App, actors: Vec<ActorConfig>) -> Result<(Vec<Addr>, Vec<Cw20Coin>)> {
        let pairs = actors
            .into_iter()
            .map(|lp| -> Result<_> {
                app.init_bank_balance(&lp.addr, coins(lp.btc, "btc"))
                    .map_err(|err| anyhow!(err))?;

                let cash = Cw20Coin {
                    address: lp.addr.to_string(),
                    amount: Uint128::new(lp.cash),
                };

                Ok((lp.addr, cash))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(pairs.into_iter().unzip())
    }

    /// Initializes actors and returns proper test suite helper
    fn init(self) -> Result<Suite> {
        let mut app = mock_app();

        let cw20_id = app.store_code(contract_cw20());
        let pair_id = app.store_code(contract_pair());

        let admin = Addr::unchecked("admin");

        // Initialize actors
        let (lps, lp_balances) = Self::init_actors(&mut app, self.lps)?;
        let (traders, traders_balances) = Self::init_actors(&mut app, self.traders)?;

        let initial_balances = [lp_balances, traders_balances].concat();
        let cash = app
            .instantiate_contract(
                cw20_id,
                admin.clone(),
                &cw20_base::msg::InstantiateMsg {
                    name: "Cash Money".to_owned(),
                    symbol: "cash".to_owned(),
                    decimals: 2,
                    initial_balances,
                    mint: None,
                },
                &[],
                "Cash",
            )
            .map_err(|err| anyhow!(err))?;

        let pair = app
            .instantiate_contract(
                pair_id,
                admin.clone(),
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
            .map_err(|err| anyhow!(err))?;

        let PairInfo {
            liquidity_token: lt,
            ..
        } = app
            .wrap()
            .query_wasm_smart(pair.clone(), &QueryMsg::Pair {})
            .map_err(|err| anyhow!(err))?;

        Ok(Suite {
            app,
            admin,
            cash,
            pair,
            lt,
            traders,
            lps,
        })
    }
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
    let res: std::result::Result<SimulationResponse, _> =
        app.wrap().query_wasm_smart(&pair_addr, &query_msg);
    let err = res.unwrap_err();
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
    let res: std::result::Result<SimulationResponse, _> =
        app.wrap().query_wasm_smart(&pair_addr, &query_msg);
    let err = res.unwrap_err();
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
    // Initialize suite:
    // liquidity provider (lp): 2000btc + 6000cash
    // trader: 1000btc
    // trader_recv: nothing (should receive funds later)
    let mut suite = SuiteConfig::new()
        .with_liquidity_provider("liquidity-provider", 2000, 6000)
        .with_trader("trader", 1000, 0)
        .with_trader("trader-recv", 0, 0)
        .init()
        .unwrap();

    let (lp, trader, trader_recv, pair) = (
        suite.lps[0].clone(),
        suite.traders[0].clone(),
        suite.traders[1].clone(),
        suite.pair.clone(),
    );

    suite.provide_liquidity(&lp, 2000, 6000, None).unwrap();

    // liquidity provider -> pair: 6000btc + 2000cash
    // liquidity provider: 3464lt minted by pair (provided sqrt(6000 [btc] * 2000 [cash])
    suite
        .assert_balances(&lp, 0, 0, 3464)
        .assert_balances(&trader, 1000, 0, 0)
        .assert_balances(&trader_recv, 0, 0, 0)
        .assert_balances(&pair, 2000, 6000, 0);

    suite.swap_btc(&trader, 1000, None, None, None).unwrap();

    // trader -> pair: 1000btc
    // pair -> trader: 1994cash, explanaction:
    //   btc to be left on contract: 6000 * 2000 / (2000 + 1000) = 4000
    //   btc to be paid out: 6000 - 2000 = 4000
    //   btc to be paid out after commission: 2000 - 2000 * 0.03% = 2000 - 2000 * 0.997 = 1994
    suite
        .assert_balances(&lp, 0, 0, 3464)
        .assert_balances(&trader, 0, 1994, 0)
        .assert_balances(&trader_recv, 0, 0, 0)
        .assert_balances(&pair, 3000, 4006, 0);

    suite
        .swap_cash(&trader, 1000, None, None, trader_recv.clone())
        .unwrap();

    // trader -> pair: 1000cash
    // pair -> trader_recv: 599 cash, explanation:
    //   cash to be left on contract: 3000 * 4006 / (4006 + 1000) = 2400
    //   cash to be paid out: 3000 - 2400 = 600
    //   cash to be paid out after commission: 600 - 600 * 0.003 = 599
    suite
        .assert_balances(&lp, 0, 0, 3464)
        .assert_balances(&trader, 0, 994, 0)
        .assert_balances(&trader_recv, 599, 0, 0)
        .assert_balances(&pair, 2401, 5006, 0);

    suite.withdraw_liquidity(&lp, 3464).unwrap();

    // liquidity provider -> pair: 3464lt (all burned in pair)
    // pair -> liquidity provider: 2401btc + 5006cash (whole pair - lp owned 100% of lt)
    //
    // Note, that lp provided initially 6000btc and 2000cash, 6000 * 2000 = 12*10^6
    // Lp payed out 2401btc, and 5006 cash, 2401 * 5006 > 12 * 10^6
    // 1btc and 6cash is what lp earned on commissions, as 2400 * 5000 = 12*10^6
    suite
        .assert_balances(&lp, 2401, 5006, 0)
        .assert_balances(&trader, 0, 994, 0)
        .assert_balances(&trader_recv, 599, 0, 0)
        .assert_balances(&pair, 0, 0, 0);
}
