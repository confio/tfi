use cw20_base::msg::InstantiateMarketingInfo;

use cosmwasm_std::testing::{mock_env, MockApi, MockStorage};
use cosmwasm_std::{to_binary, Addr, Binary, Empty, Event, Response, StdError, Uint128};
use cw20::{Cw20Coin, Cw20Contract, Cw20ReceiveMsg, MinterResponse, TokenInfoResponse};
use cw4::{Cw4Contract, Member};
use cw4_group::msg::ExecuteMsg as Cw4ExecuteMsg;
use cw_multi_test::{App, BankKeeper, Contract, ContractWrapper, Executor};

use crate::msg::{ExecuteMsg, InstantiateMsg};

use anyhow::{anyhow, Result};
use derivative::Derivative;

mod receiver {
    // Implementation of artificial contract for receiving cw20 messages

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
}

pub struct ReceiverContract(Addr);

impl ReceiverContract {
    /// Helper for instantiating the contract
    pub fn init(app: &mut App, owner: Addr) -> Result<Self> {
        let id = app.store_code(receiver::contract());
        app.instantiate_contract(
            id,
            owner,
            &receiver::InstantiateMsg {},
            &[],
            "Receiver",
            None,
        )
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

fn mock_app() -> App {
    let env = mock_env();
    let api = MockApi::default();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();

    App::new(api, env.block, bank, storage)
}

fn contract_group() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw4_group::contract::execute,
        cw4_group::contract::instantiate,
        cw4_group::contract::query,
    );
    Box::new(contract)
}

fn contract_cw20() -> Box<dyn Contract<Empty>> {
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
pub struct Suite {
    /// Application mock
    #[derivative(Debug = "ignore")]
    pub app: App,
    /// Special account for performing administrative execution
    pub owner: Addr,
    /// Members of whitelist
    pub members: Vec<Addr>,
    /// Address allowed to mint new tokens if any
    pub minter: Option<Addr>,
    /// cw4 whitelist contract address
    pub whitelist: Cw4Contract,
    /// dso-token cash contract address
    pub cash: Cw20Contract,
    /// events collected from executions, for verifiaction purposes
    pub events: Vec<Event>,
}

/// Utility functions sending messages to execute contracts.
impl Suite {
    /// Adds member to whitelist
    pub fn add_member(&mut self, addr: &Addr, weight: u64) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                self.owner.clone(),
                self.whitelist.addr(),
                &Cw4ExecuteMsg::UpdateMembers {
                    add: vec![Member {
                        addr: addr.to_string(),
                        weight,
                    }],
                    remove: vec![],
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Removes member from whitelist
    pub fn remove_member(&mut self, addr: &Addr) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                self.owner.clone(),
                self.whitelist.addr(),
                &Cw4ExecuteMsg::UpdateMembers {
                    add: vec![],
                    remove: vec![addr.to_string()],
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes transfer on `cash` contract
    pub fn transfer(
        &mut self,
        executor: &Addr,
        recipient: &Addr,
        amount: u128,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::Transfer {
                    recipient: recipient.to_string(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes burn on `cash` contract
    pub fn burn(&mut self, executor: &Addr, amount: u128) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::Burn {
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes send on `cash` contract
    pub fn send(
        &mut self,
        executor: &Addr,
        recipient: &Addr,
        amount: u128,
        msg: impl Into<Binary>,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::Send {
                    contract: recipient.to_string(),
                    amount: amount.into(),
                    msg: msg.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes mint on `cash` contract
    pub fn mint(&mut self, executor: &Addr, recipient: &Addr, amount: u128) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::Mint {
                    recipient: recipient.to_string(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes increasing allowance on `cash` contract
    pub fn increase_allowance(
        &mut self,
        executor: &Addr,
        spender: &Addr,
        amount: u128,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::IncreaseAllowance {
                    spender: spender.to_string(),
                    amount: amount.into(),
                    expires: None,
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes decreasing allowance on `cash` contract
    pub fn decrease_allowance(
        &mut self,
        executor: &Addr,
        spender: &Addr,
        amount: u128,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::DecreaseAllowance {
                    spender: spender.to_string(),
                    amount: amount.into(),
                    expires: None,
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes transfer from on `cash` contract
    pub fn transfer_from(
        &mut self,
        executor: &Addr,
        owner: &Addr,
        recipient: &Addr,
        amount: u128,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::TransferFrom {
                    owner: owner.to_string(),
                    recipient: recipient.to_string(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes burn from on `cash` contract
    pub fn burn_from(&mut self, executor: &Addr, owner: &Addr, amount: u128) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::BurnFrom {
                    owner: owner.to_string(),
                    amount: amount.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes send from on `cash` contract
    pub fn send_from(
        &mut self,
        executor: &Addr,
        owner: &Addr,
        recipient: &Addr,
        amount: u128,
        msg: impl Into<Binary>,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::SendFrom {
                    owner: owner.to_string(),
                    contract: recipient.to_string(),
                    amount: amount.into(),
                    msg: msg.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Executes redeem on `cash`
    pub fn redeem(
        &mut self,
        executor: &Addr,
        amount: u128,
        code: impl Into<String>,
        sender: impl Into<Option<String>>,
        memo: impl Into<String>,
    ) -> Result<&mut Self> {
        let resp = self
            .app
            .execute_contract(
                executor.clone(),
                self.cash.addr(),
                &ExecuteMsg::Redeem {
                    amount: amount.into(),
                    code: code.into(),
                    sender: sender.into().map(Into::into),
                    memo: memo.into(),
                },
                &[],
            )
            .map_err(|err| anyhow!(err))?;

        self.events.extend(resp.events);

        Ok(self)
    }

    /// Return cash contract metadata
    pub fn meta(&self) -> Result<TokenInfoResponse> {
        self.cash.meta(&self.app).map_err(|err| anyhow!(err))
    }

    /// Return given address cash balance
    pub fn balance(&self, account: &Addr) -> Result<u128> {
        self.cash
            .balance(&self.app, account)
            .map(Into::into)
            .map_err(|err| anyhow!(err))
    }

    /// Returns cash total supply
    pub fn total_supply(&self) -> Result<u128> {
        Ok(self.meta()?.total_supply.into())
    }

    /// Returns allowance on cash
    pub fn allowance(&self, owner: &Addr, spender: &Addr) -> Result<u128> {
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
pub struct Config {
    /// Initial members of whitelist
    members: Vec<MemberConfig>,
    /// Initial marketing info
    marketing: Option<InstantiateMarketingInfo>,
    /// Address allowed to ming new tokens. Not neccessary member of a whitelist.
    minter: Option<MinterResponse>,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_member(mut self, addr: &str, cash: u128, weight: u64) -> Self {
        self.members.push(MemberConfig {
            addr: addr.to_owned(),
            cash,
            weight,
        });

        self
    }

    pub fn with_minter(mut self, addr: &str, cap: impl Into<Option<u128>>) -> Self {
        self.minter = Some(MinterResponse {
            minter: addr.to_owned(),
            cap: cap.into().map(Uint128::new),
        });

        self
    }

    pub fn init(self) -> Result<Suite> {
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
            events: Vec::new(),
        })
    }
}
