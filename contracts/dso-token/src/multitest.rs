use cw20_base::msg::InstantiateMarketingInfo;

use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{to_binary, Addr, Binary, Empty, Response, StdError, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ReceiveMsg, MinterResponse, TokenInfoResponse};
use cw4::{Cw4Contract, Member};
use cw4_group::msg::ExecuteMsg as Cw4ExecuteMsg;
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};

use crate::msg::{ExecuteMsg, InstantiateMsg, IsWhitelistedResponse, QueryMsg, WhitelistResponse};

use anyhow::{anyhow, Result};
use derivative::Derivative;

mod receiver {
    // Implementation of very much artificial contract for receiving cw20 messages

    use super::*;
    use cosmwasm_std::{Deps, DepsMut, Env, MessageInfo};
    use cw_storage_plus::Item;
    use serde::{Deserialize, Serialize};

    pub const MESSAGES: Item<Vec<Cw20ReceiveMsg>> = Item::new("messages");

    #[derive(Serialize, Deserialize)]
    pub struct InstantiateMsg {}

    #[derive(Serialize, Deserialize)]
    pub struct QueryMsg {}

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        Receive(Cw20ReceiveMsg),
    }

    impl From<ExecuteMsg> for Cw20ReceiveMsg {
        fn from(src: ExecuteMsg) -> Self {
            match src {
                ExecuteMsg::Receive(msg) => msg,
            }
        }
    }

    fn instantiate(
        deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        _msg: InstantiateMsg,
    ) -> Result<Response, StdError> {
        MESSAGES.save(deps.storage, &vec![])?;
        Ok(Response::default())
    }

    fn execute(
        deps: DepsMut,
        _env: Env,
        _info: MessageInfo,
        msg: ExecuteMsg,
    ) -> Result<Response, StdError> {
        MESSAGES.update(deps.storage, |mut messages| -> Result<_, StdError> {
            messages.push(msg.into());
            Ok(messages)
        })?;

        Ok(Response::new())
    }

    fn query(deps: Deps, _env: Env, _msg: QueryMsg) -> Result<Binary, StdError> {
        to_binary(&MESSAGES.load(deps.storage)?)
    }

    pub fn contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(execute, instantiate, query);
        Box::new(contract)
    }

    pub struct ReceiverContract(Addr);

    impl ReceiverContract {
        /// Helper for instantiating the contract
        pub fn init(app: &mut App, owner: Addr) -> Result<Self> {
            let id = app.store_code(contract());
            app.instantiate_contract(id, owner, &InstantiateMsg {}, &[], "Receiver", None)
                .map(Self)
                .map_err(|err| anyhow!(err))
        }

        pub fn addr(&self) -> Addr {
            self.0.clone()
        }

        /// Helper for querying for stored messages
        pub fn messages(&self, app: &App) -> Result<Vec<Cw20ReceiveMsg>> {
            app.wrap()
                .query_wasm_smart(&self.0, &receiver::QueryMsg {})
                .map_err(|err| anyhow!(err))
        }
    }
}

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

/// Testing environment with dso-token "cash", and configured members
#[derive(Derivative)]
#[derivative(Debug)]
struct Suite {
    /// Application mock
    #[derivative(Debug = "ignore")]
    app: App,
    /// Special account for performing administrative execution
    owner: Addr,
    /// Members of whitelist
    members: Vec<Addr>,
    /// Address allowed to mint new tokens if any
    minter: Option<Addr>,
    /// cw4 whitelist contract address
    whitelist: Cw4Contract,
    /// dso-token cash contract address
    cash: Cw20Contract,
}

/// Utility functions sending messages to execute contracts.
impl Suite {
    /// Adds member to whitelist
    fn add_member(&mut self, addr: impl Into<String>, weight: u64) -> Result<&mut Self> {
        self.app
            .execute_contract(
                self.owner.clone(),
                self.whitelist.addr(),
                &Cw4ExecuteMsg::UpdateMembers {
                    add: vec![Member {
                        addr: addr.into(),
                        weight,
                    }],
                    remove: vec![],
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Removes member from whitelist
    fn remove_member(&mut self, addr: impl Into<String>) -> Result<&mut Self> {
        self.app
            .execute_contract(
                self.owner.clone(),
                self.whitelist.addr(),
                &Cw4ExecuteMsg::UpdateMembers {
                    add: vec![],
                    remove: vec![addr.into()],
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes transfer on `cash` contract
    fn transfer(
        &mut self,
        executor: Addr,
        recipient: impl Into<String>,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::Transfer {
                    recipient: recipient.into(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes burn on `cash` contract
    fn burn(&mut self, executor: Addr, amount: u128) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::Burn {
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes send on `cash` contract
    fn send(
        &mut self,
        executor: Addr,
        recipient: impl Into<String>,
        amount: u128,
        msg: impl Into<Binary>,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::Send {
                    contract: recipient.into(),
                    amount: amount.into(),
                    msg: msg.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes mint on `cash` contract
    fn mint(
        &mut self,
        executor: Addr,
        recipient: impl Into<String>,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::Mint {
                    recipient: recipient.into(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes increasing allowance on `cash` contract
    fn increase_allowance(
        &mut self,
        executor: Addr,
        spender: impl Into<String>,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::IncreaseAllowance {
                    spender: spender.into(),
                    amount: amount.into(),
                    expires: None,
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes decreasing allowance on `cash` contract
    fn decrease_allowance(
        &mut self,
        executor: Addr,
        spender: impl Into<String>,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::DecreaseAllowance {
                    spender: spender.into(),
                    amount: amount.into(),
                    expires: None,
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes transfer from on `cash` contract
    fn transfer_from(
        &mut self,
        executor: Addr,
        owner: impl Into<String>,
        recipient: impl Into<String>,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::TransferFrom {
                    owner: owner.into(),
                    recipient: recipient.into(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes burn from on `cash` contract
    fn burn_from(
        &mut self,
        executor: Addr,
        owner: impl Into<String>,
        amount: u128,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::BurnFrom {
                    owner: owner.into(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Executes send from on `cash` contract
    fn send_from(
        &mut self,
        executor: Addr,
        owner: impl Into<String>,
        recipient: impl Into<String>,
        amount: u128,
        msg: impl Into<Binary>,
    ) -> Result<&mut Self> {
        self.app
            .execute_contract(
                executor,
                self.cash.addr(),
                &ExecuteMsg::SendFrom {
                    owner: owner.into(),
                    contract: recipient.into(),
                    amount: amount.into(),
                    msg: msg.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        Ok(self)
    }

    /// Return cash contract metadata
    fn meta(&self) -> Result<TokenInfoResponse> {
        self.cash.meta(&self.app).map_err(|err| anyhow!(err))
    }

    /// Return given address cash balance
    fn balance(&self, account: &Addr) -> Result<u128> {
        self.cash
            .balance(&self.app, account)
            .map(Into::into)
            .map_err(|err| anyhow!(err))
    }

    /// Returns cash total supply
    fn total_supply(&self) -> Result<u128> {
        Ok(self.meta()?.total_supply.into())
    }

    /// Returns allowance on cash
    fn allowance(&self, owner: &Addr, spender: &Addr) -> Result<u128> {
        self.cash
            .allowance(&self.app, owner.clone(), spender.clone())
            .map(|allowance| allowance.allowance.into())
            .map_err(|err| anyhow!(err))
    }
}

/// Configuration of single whitelist member
struct MemberConfig {
    /// Member address
    addr: String,
    /// Innitial cash amount
    cash: u128,
    /// Member weight in whitelist
    weight: u64,
}

#[derive(Default)]
struct SuiteConfig {
    /// Initial members of whitelist
    members: Vec<MemberConfig>,
    /// Initial marketing info
    marketing: Option<InstantiateMarketingInfo>,
    /// Address allowed to ming new tokens. Not neccessary member of a whitelist.
    minter: Option<MinterResponse>,
}

impl SuiteConfig {
    fn new() -> Self {
        Self::default()
    }

    fn with_member(mut self, addr: &str, cash: u128, weight: u64) -> Self {
        self.members.push(MemberConfig {
            addr: addr.to_owned(),
            cash,
            weight,
        });

        self
    }

    fn with_minter(mut self, addr: &str, cap: impl Into<Option<u128>>) -> Self {
        self.minter = Some(MinterResponse {
            minter: addr.to_owned(),
            cap: cap.into().map(Uint128::new),
        });

        self
    }

    fn init(self) -> Result<Suite> {
        let mut app = mock_app();
        let owner = Addr::unchecked("owner");
        let cw4_id = app.store_code(contract_group());
        let cw20_id = app.store_code(contract_cw20());

        let (members, initial_cash): (Vec<_>, Vec<_>) = self
            .members
            .into_iter()
            .map(|member| -> Result<_> {
                let initial_cash = Cw20Coin {
                    address: member.addr.to_string(),
                    amount: Uint128::new(member.cash),
                };
                let member = Member {
                    addr: member.addr.to_string(),
                    weight: member.weight,
                };
                Ok((member, initial_cash))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();

        let whitelist = app
            .instantiate_contract(
                cw4_id,
                owner.clone(),
                &cw4_group::msg::InstantiateMsg {
                    admin: Some(owner.to_string()),
                    members: members.clone(),
                },
                &[],
                "Whitelist",
                None,
            )
            .unwrap();

        let minter = self
            .minter
            .as_ref()
            .map(|minter| Addr::unchecked(&minter.minter));

        let cash = app
            .instantiate_contract(
                cw20_id,
                owner.clone(),
                &InstantiateMsg {
                    name: "Cash Token".to_owned(),
                    symbol: "CASH".to_owned(),
                    decimals: 9,
                    initial_balances: initial_cash,
                    mint: self.minter,
                    marketing: self.marketing,
                    whitelist_group: whitelist.to_string(),
                },
                &[],
                "Cash",
                None,
            )
            .unwrap();

        let members = members
            .into_iter()
            .map(|member| Addr::unchecked(member.addr))
            .collect();

        Ok(Suite {
            app,
            owner,
            members,
            minter,
            whitelist: Cw4Contract(whitelist),
            cash: Cw20Contract(cash),
        })
    }
}

#[test]
fn proper_instantiation() {
    let suite = SuiteConfig::new()
        .with_member("member1", 1000, 10)
        .with_member("member2", 2000, 20)
        .init()
        .unwrap();

    assert_eq!(
        suite.meta().unwrap(),
        TokenInfoResponse {
            name: "Cash Token".to_string(),
            symbol: "CASH".to_string(),
            decimals: 9,
            total_supply: Uint128::new(3000),
        }
    );
    assert_eq!(suite.balance(&suite.members[0]).unwrap(), 1000);
    assert_eq!(suite.balance(&suite.members[1]).unwrap(), 2000);
}

#[test]
fn transfer() {
    let mut suite = SuiteConfig::new()
        .with_member("member1", 1000, 10)
        .with_member("member2", 2000, 20)
        .init()
        .unwrap();
    let (member1, member2) = (suite.members[0].clone(), suite.members[1].clone());

    // send to whitelisted member works
    suite
        .transfer(member1.clone(), member2.clone(), 500)
        .unwrap();

    assert_eq!(suite.balance(&member1).unwrap(), 500);
    assert_eq!(suite.balance(&member2).unwrap(), 2500);

    // send to non-whitelisted address fails
    suite
        .transfer(member1.clone(), "non-member", 500)
        .unwrap_err();

    assert_eq!(suite.balance(&member1).unwrap(), 500);
    assert_eq!(suite.balance(&member2).unwrap(), 2500);
}

#[test]
fn burn() {
    let mut suite = SuiteConfig::new()
        .with_member("member", 1000, 10)
        .init()
        .unwrap();
    let member = suite.members[0].clone();

    // whitelisted member can burn his own tokens
    suite.burn(member.clone(), 500).unwrap();

    assert_eq!(suite.balance(&suite.members[0]).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);

    // non whitelisted can't burn tokens
    suite
        .remove_member(member.clone())
        .unwrap()
        .burn(member.clone(), 500)
        .unwrap_err();

    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);
}

#[test]
fn send() {
    // Testing send is tricky, as there is need for some contract which is able to receive
    // messages.
    let mut suite = SuiteConfig::new()
        .with_member("member", 1000, 10)
        .init()
        .unwrap();
    let member = suite.members[0].clone();

    // Instantiate receiver contract
    let receiver = receiver::ReceiverContract::init(&mut suite.app, suite.owner.clone()).unwrap();

    // send to non-whitelisted address fails
    suite
        .send(member.clone(), receiver.addr(), 500, "msg".as_bytes())
        .unwrap_err();

    assert_eq!(suite.balance(&member).unwrap(), 1000);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 0);
    assert_eq!(receiver.messages(&suite.app).unwrap(), vec![]);

    // send to whitelisted address works
    suite
        .add_member(receiver.addr(), 10)
        .unwrap()
        .send(member.clone(), receiver.addr(), 500, "'msg2'".as_bytes())
        .unwrap();

    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 500);
    assert_eq!(
        receiver.messages(&suite.app).unwrap(),
        vec![Cw20ReceiveMsg {
            sender: member.to_string(),
            amount: Uint128::new(500),
            msg: "'msg2'".as_bytes().into()
        }]
    );

    // sned by non-whitelisted owner fails
    suite
        .remove_member(member.clone())
        .unwrap()
        .send(member.clone(), receiver.addr(), 500, "msg3".as_bytes())
        .unwrap_err();

    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 500);
    assert_eq!(
        receiver.messages(&suite.app).unwrap(),
        vec![Cw20ReceiveMsg {
            sender: member.to_string(),
            amount: Uint128::new(500),
            msg: "'msg2'".as_bytes().into()
        }]
    );
}

#[test]
fn mint() {
    let mut suite = SuiteConfig::new()
        .with_minter("minter", None)
        .with_member("member", 0, 10)
        .init()
        .unwrap();
    let (minter, member) = (suite.minter.clone().unwrap(), suite.members[0].clone());

    // mint by non-whitelisted minter fails
    suite.mint(minter.clone(), member.clone(), 500).unwrap_err();
    assert_eq!(suite.total_supply().unwrap(), 0);

    // mint by whitelisted minter to whitelisted member works
    suite
        .add_member(minter.clone(), 20)
        .unwrap()
        .mint(minter.clone(), member.clone(), 500)
        .unwrap();
    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);

    // mint to non-whitelisted addres fails
    suite.mint(minter, "non-member", 500).unwrap_err();
    assert_eq!(suite.total_supply().unwrap(), 500);
}

#[test]
fn increase_allowance() {
    let mut suite = SuiteConfig::new()
        .with_member("member1", 1000, 10)
        .init()
        .unwrap();
    let member1 = suite.members[0].clone();
    let member2 = Addr::unchecked("member2");

    // whitelisted member can increse allowance on his own tokens
    suite
        .increase_allowance(member1.clone(), member2.to_string(), 500)
        .unwrap();

    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);

    // non whitelisted can't increase allowance
    suite
        .remove_member(member1.clone())
        .unwrap()
        .increase_allowance(member1.clone(), member2.clone(), 500)
        .unwrap_err();

    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);
}

#[test]
fn decrease_allowance() {
    let mut suite = SuiteConfig::new()
        .with_member("member1", 1000, 10)
        .init()
        .unwrap();
    let member1 = suite.members[0].clone();
    let member2 = Addr::unchecked("member2");

    // setup initial allowance
    suite
        .increase_allowance(member1.clone(), member2.clone(), 1000)
        .unwrap();

    // whitelisted member can decrease allowance on his own tokens
    suite
        .decrease_allowance(member1.clone(), member2.clone(), 500)
        .unwrap();
    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);

    // non whitelisted can't decrease allowance
    suite
        .remove_member(member1.clone())
        .unwrap()
        .decrease_allowance(member1.clone(), member2.clone(), 500)
        .unwrap_err();
    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);
}

#[test]
fn transfer_from() {
    let mut suite = SuiteConfig::new()
        .with_member("member", 2000, 10)
        .with_member("receiver", 2000, 20)
        .with_member("spender", 0, 30)
        .init()
        .unwrap();
    let (member, receiver, spender) = (
        suite.members[0].clone(),
        suite.members[1].clone(),
        suite.members[2].clone(),
    );
    let non_member = Addr::unchecked("non-member");

    // setup allowance
    suite
        .increase_allowance(member.clone(), spender.clone(), 1000)
        .unwrap()
        .increase_allowance(member.clone(), non_member.clone(), 1000)
        .unwrap();

    // send when all whitelisted member works
    suite
        .transfer_from(spender.clone(), member.clone(), receiver.clone(), 500)
        .unwrap();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&receiver).unwrap(), 2500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);

    // send to non-whitelisted address fails
    suite
        .transfer_from(spender.clone(), member.clone(), non_member.clone(), 500)
        .unwrap_err();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&non_member).unwrap(), 0);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);

    // send by non-whitelisted allowed address fails
    suite
        .transfer_from(non_member.clone(), member.clone(), receiver.clone(), 500)
        .unwrap_err();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&receiver).unwrap(), 2500);
    assert_eq!(suite.allowance(&member, &non_member).unwrap(), 1000);

    // send by non-whitelisted allowed address fails
    suite
        .remove_member(member.clone())
        .unwrap()
        .transfer_from(spender.clone(), member.clone(), receiver.clone(), 500)
        .unwrap_err();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&receiver).unwrap(), 2500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
}

#[test]
fn burn_from() {
    let mut suite = SuiteConfig::new()
        .with_member("member", 2000, 10)
        .with_member("spender", 0, 20)
        .init()
        .unwrap();
    let (member, spender) = (suite.members[0].clone(), suite.members[1].clone());
    let non_member = Addr::unchecked("non-member");

    // setup allowances
    suite
        .increase_allowance(member.clone(), spender.clone(), 1000)
        .unwrap()
        .increase_allowance(member.clone(), non_member.clone(), 1000)
        .unwrap();

    // whitelisted member can burn tokens he is allowed on another whitelisted address
    suite
        .burn_from(spender.clone(), member.clone(), 500)
        .unwrap();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 1500);

    // non whitelisted can't burn tokens
    suite
        .burn_from(non_member.clone(), member.clone(), 500)
        .unwrap_err();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.allowance(&member, &non_member).unwrap(), 1000);
    assert_eq!(suite.total_supply().unwrap(), 1500);

    // cannot burn tokens from non-whitelisted account
    suite
        .remove_member(member.clone())
        .unwrap()
        .burn_from(spender.clone(), member.clone(), 500)
        .unwrap_err();
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 1500);
}

#[test]
fn send_from() {
    // Testing send is tricky, as there is need for some contract which is able to receive
    // messages.
    let mut suite = SuiteConfig::new()
        .with_member("member", 1000, 10)
        .with_member("spender", 0, 20)
        .init()
        .unwrap();
    let (member, spender) = (suite.members[0].clone(), suite.members[1].clone());
    let non_member = Addr::unchecked("non-member");

    // Instantiate receiver contract
    let receiver = receiver::ReceiverContract::init(&mut suite.app, suite.owner.clone()).unwrap();

    // Set up allowances
    suite
        .increase_allowance(member.clone(), spender.clone(), 500)
        .unwrap()
        .increase_allowance(member.clone(), non_member.clone(), 500)
        .unwrap();

    // send to non-whitelisted address fails
    suite
        .send_from(
            spender.clone(),
            member.clone(),
            receiver.addr(),
            500,
            "msg".as_bytes(),
        )
        .unwrap_err();

    assert_eq!(suite.balance(&member).unwrap(), 1000);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 0);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
    assert_eq!(receiver.messages(&suite.app).unwrap(), vec![]);

    // send when all whitelisted works
    suite
        .add_member(receiver.addr(), 10)
        .unwrap()
        .send_from(
            spender.clone(),
            member.clone(),
            receiver.addr(),
            500,
            "'msg2'".as_bytes(),
        )
        .unwrap();

    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 0);
    assert_eq!(
        receiver.messages(&suite.app).unwrap(),
        vec![Cw20ReceiveMsg {
            sender: spender.to_string(),
            amount: Uint128::new(500),
            msg: "'msg2'".as_bytes().into()
        }]
    );

    // send by non-whitelisted spender fails
    suite
        .send_from(
            non_member.clone(),
            member.clone(),
            receiver.addr(),
            500,
            "msg3".as_bytes(),
        )
        .unwrap_err();

    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 500);
    assert_eq!(suite.allowance(&member, &non_member).unwrap(), 500);
    assert_eq!(
        receiver.messages(&suite.app).unwrap(),
        vec![Cw20ReceiveMsg {
            sender: spender.to_string(),
            amount: Uint128::new(500),
            msg: "'msg2'".as_bytes().into()
        }]
    );

    // send from non-whitelisted owner fails
    suite
        .remove_member(member.clone())
        .unwrap()
        .send_from(
            spender.clone(),
            member.clone(),
            receiver.addr(),
            500,
            "msg3".as_bytes(),
        )
        .unwrap_err();
    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 0);
    assert_eq!(
        receiver.messages(&suite.app).unwrap(),
        vec![Cw20ReceiveMsg {
            sender: spender.to_string(),
            amount: Uint128::new(500),
            msg: "'msg2'".as_bytes().into()
        }]
    );
}

#[test]
fn whitelist() {
    let suite = SuiteConfig::new()
        .with_member("member", 1000, 10)
        .init()
        .unwrap();

    let (app, cash, member) = (&suite.app, &suite.cash, &suite.members[0]);

    let whitelist: WhitelistResponse = app
        .wrap()
        .query_wasm_smart(&cash.addr(), &QueryMsg::Whitelist {})
        .unwrap();
    assert_eq!(whitelist.address, suite.whitelist.addr());

    let is_whitelisted: IsWhitelistedResponse = app
        .wrap()
        .query_wasm_smart(
            &cash.addr(),
            &QueryMsg::IsWhitelisted {
                address: member.to_string(),
            },
        )
        .unwrap();
    assert!(is_whitelisted.whitelisted);

    let is_whitelisted: IsWhitelistedResponse = app
        .wrap()
        .query_wasm_smart(
            &cash.addr(),
            &QueryMsg::IsWhitelisted {
                address: "non-member".to_owned(),
            },
        )
        .unwrap();
    assert!(!is_whitelisted.whitelisted);
}
