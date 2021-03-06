mod suite;

use cosmwasm_std::{Addr, Deps, Event, Uint128};
use cw20::{Cw20ReceiveMsg, TokenInfoResponse};

use crate::contract::{verify_sender_and_addresses_on_whitelist, verify_sender_on_whitelist};
use crate::error::ContractError;
use crate::msg::{IsWhitelistedResponse, QueryMsg, WhitelistResponse};

use crate::state::WHITELIST;
use anyhow::Error;
use cosmwasm_std::testing::{MockApi, MockStorage};

/// Compares if error is as expected
///
/// Unfortunately, error types information is lost, as in multitest every error is just converted
/// to its string representation. To solve this issue and still be able to reasonably test returned
/// error, but to avoid maintaining error string validation, errors are passed strongly typed, but
/// verified on their representation level. Additionally when error doesn't match, the actual
/// error is printed in debug form so additional `anyhow` information is displayed.
#[track_caller]
fn assert_error(err: Error, expected: ContractError) {
    assert_eq!(
        err.root_cause().to_string(),
        expected.to_string(),
        "received error {:?} while expected {:?}",
        err,
        expected
    );
}

#[test]
fn proper_instantiation() {
    let suite = suite::Config::new()
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
    let mut suite = suite::Config::new()
        .with_member("member1", 1000, 10)
        .with_member("member2", 2000, 20)
        .init()
        .unwrap();
    let (member1, member2) = (suite.members[0].clone(), suite.members[1].clone());
    let non_member = Addr::unchecked("non-member");

    // send to whitelisted member works
    suite.transfer(&member1, &member2, 500).unwrap();

    assert_eq!(suite.balance(&member1).unwrap(), 500);
    assert_eq!(suite.balance(&member2).unwrap(), 2500);

    // send to non-whitelisted address fails
    let err = suite.transfer(&member1, &non_member, 500).unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member1).unwrap(), 500);
    assert_eq!(suite.balance(&member2).unwrap(), 2500);
}

#[test]
fn burn() {
    let mut suite = suite::Config::new()
        .with_member("member", 1000, 10)
        .init()
        .unwrap();
    let member = suite.members[0].clone();

    // whitelisted member can burn his own tokens
    suite.burn(&member, 500).unwrap();

    assert_eq!(suite.balance(&suite.members[0]).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);

    // non whitelisted can't burn tokens
    suite.remove_member(&member).unwrap();
    let err = suite.burn(&member, 500).unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);
}

#[test]
fn whitelist_works() {
    let suite = suite::Config::new()
        .with_member("member", 1000, 10)
        .with_member("member2", 1000, 0)
        .init()
        .unwrap();
    let member = suite.members[0].clone();
    let member2 = suite.members[1].clone();
    let non_member = Addr::unchecked("nonmember");

    // set our local data
    let api = MockApi::default();
    let mut storage = MockStorage::new();
    WHITELIST.save(&mut storage, &suite.whitelist).unwrap();
    let deps = Deps {
        storage: &storage,
        api: &api,
        // querier is pointing to the app (with other contract initialized) for now
        // we can remove the need for multi-test later
        querier: suite.app.wrap(),
    };

    // sender whitelisted regardless of weight
    verify_sender_on_whitelist(deps, &member).unwrap();
    verify_sender_on_whitelist(deps, &member2).unwrap();
    let err = verify_sender_on_whitelist(deps, &non_member).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    verify_sender_and_addresses_on_whitelist(deps, &member, &[member2.as_str()]).unwrap();
}

#[test]
fn send() {
    // Testing send is tricky, as there is need for some contract which is able to receive
    // messages.
    let mut suite = suite::Config::new()
        .with_member("member", 1000, 10)
        .init()
        .unwrap();
    let member = suite.members[0].clone();

    // Instantiate receiver contract
    let receiver = suite::ReceiverContract::init(&mut suite.app, suite.owner.clone()).unwrap();

    // send to non-whitelisted address fails
    let err = suite
        .send(&member, &receiver.addr(), 500, "msg".as_bytes())
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1000);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 0);
    assert_eq!(receiver.messages(&suite.app).unwrap(), vec![]);

    // send to whitelisted address works
    suite.add_member(&receiver.addr(), 10).unwrap();
    suite
        .send(&member, &receiver.addr(), 500, "'msg2'".as_bytes())
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
    suite.remove_member(&member).unwrap();
    let err = suite
        .send(&member, &receiver.addr(), 500, "msg3".as_bytes())
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
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
    let mut suite = suite::Config::new()
        .with_minter("minter", None)
        .with_member("member", 0, 10)
        .init()
        .unwrap();
    let (minter, member) = (suite.minter.clone().unwrap(), suite.members[0].clone());
    let non_member = Addr::unchecked("non-member");

    // mint by non-whitelisted minter fails
    suite.mint(&minter, &member, 500).unwrap_err();

    assert_eq!(suite.total_supply().unwrap(), 0);

    // mint by whitelisted minter to whitelisted member works
    suite.add_member(&minter, 20).unwrap();
    suite.mint(&minter, &member, 500).unwrap();
    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);

    // mint to non-whitelisted addres fails
    let err = suite.mint(&minter, &non_member, 500).unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.total_supply().unwrap(), 500);
}

#[test]
fn increase_allowance() {
    let mut suite = suite::Config::new()
        .with_member("member1", 1000, 10)
        .init()
        .unwrap();
    let member1 = suite.members[0].clone();
    let member2 = Addr::unchecked("member2");

    // whitelisted member can increse allowance on his own tokens
    suite.increase_allowance(&member1, &member2, 500).unwrap();
    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);

    // non whitelisted can't increase allowance
    suite.remove_member(&member1).unwrap();
    let err = suite
        .increase_allowance(&member1, &member2, 500)
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);
}

#[test]
fn decrease_allowance() {
    let mut suite = suite::Config::new()
        .with_member("member1", 1000, 10)
        .init()
        .unwrap();
    let member1 = suite.members[0].clone();
    let member2 = Addr::unchecked("member2");

    // setup initial allowance
    suite.increase_allowance(&member1, &member2, 1000).unwrap();

    // whitelisted member can decrease allowance on his own tokens
    suite.decrease_allowance(&member1, &member2, 500).unwrap();
    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);

    // non whitelisted can't decrease allowance
    suite.remove_member(&member1).unwrap();
    let err = suite
        .decrease_allowance(&member1, &member2, 500)
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.allowance(&member1, &member2).unwrap(), 500);
}

#[test]
fn transfer_from() {
    let mut suite = suite::Config::new()
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
    suite.increase_allowance(&member, &spender, 1000).unwrap();
    suite
        .increase_allowance(&member, &non_member, 1000)
        .unwrap();

    // send when all whitelisted member works
    suite
        .transfer_from(&spender, &member, &receiver, 500)
        .unwrap();

    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&receiver).unwrap(), 2500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);

    // send to non-whitelisted address fails
    let err = suite
        .transfer_from(&spender, &member, &non_member, 500)
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&non_member).unwrap(), 0);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);

    // send by non-whitelisted allowed address fails
    let err = suite
        .transfer_from(&non_member, &member, &receiver, 500)
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&receiver).unwrap(), 2500);
    assert_eq!(suite.allowance(&member, &non_member).unwrap(), 1000);

    // send by non-whitelisted allowed address fails
    suite.remove_member(&member).unwrap();
    let err = suite
        .transfer_from(&spender, &member, &receiver, 500)
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.balance(&receiver).unwrap(), 2500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
}

#[test]
fn burn_from() {
    let mut suite = suite::Config::new()
        .with_member("member", 2000, 10)
        .with_member("spender", 0, 20)
        .init()
        .unwrap();
    let (member, spender) = (suite.members[0].clone(), suite.members[1].clone());
    let non_member = Addr::unchecked("non-member");

    // setup allowances
    suite.increase_allowance(&member, &spender, 1000).unwrap();
    suite
        .increase_allowance(&member, &non_member, 1000)
        .unwrap();

    // whitelisted member can burn tokens he is allowed on another whitelisted address
    suite.burn_from(&spender, &member, 500).unwrap();

    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 1500);

    // non whitelisted can't burn tokens
    let err = suite.burn_from(&non_member, &member, 500).unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.allowance(&member, &non_member).unwrap(), 1000);
    assert_eq!(suite.total_supply().unwrap(), 1500);

    // cannot burn tokens from non-whitelisted account
    suite.remove_member(&member).unwrap();
    let err = suite.burn_from(&spender, &member, 500).unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1500);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 1500);
}

#[test]
fn send_from() {
    // Testing send is tricky, as there is need for some contract which is able to receive
    // messages.
    let mut suite = suite::Config::new()
        .with_member("member", 1000, 10)
        .with_member("spender", 0, 20)
        .init()
        .unwrap();
    let (member, spender) = (suite.members[0].clone(), suite.members[1].clone());
    let non_member = Addr::unchecked("non-member");

    // Instantiate receiver contract
    let receiver = suite::ReceiverContract::init(&mut suite.app, suite.owner.clone()).unwrap();

    // Set up allowances
    suite.increase_allowance(&member, &spender, 500).unwrap();
    suite.increase_allowance(&member, &non_member, 500).unwrap();

    // send to non-whitelisted address fails
    let err = suite
        .send_from(&spender, &member, &receiver.addr(), 500, "msg".as_bytes())
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
    assert_eq!(suite.balance(&member).unwrap(), 1000);
    assert_eq!(suite.balance(&receiver.addr()).unwrap(), 0);
    assert_eq!(suite.allowance(&member, &spender).unwrap(), 500);
    assert_eq!(receiver.messages(&suite.app).unwrap(), vec![]);

    // send when all whitelisted works
    suite.add_member(&receiver.addr(), 10).unwrap();
    suite
        .send_from(
            &spender,
            &member,
            &receiver.addr(),
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
    let err = suite
        .send_from(
            &non_member,
            &member,
            &receiver.addr(),
            500,
            "msg3".as_bytes(),
        )
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
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
    suite.remove_member(&member).unwrap();
    let err = suite
        .send_from(&spender, &member, &receiver.addr(), 500, "msg3".as_bytes())
        .unwrap_err();

    assert_error(err, ContractError::Unauthorized {});
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
    let suite = suite::Config::new()
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

fn redeem_event(code: &str, sender: &str, amount: u128, memo: &str) -> Event {
    Event::new("wasm-redeem")
        .add_attribute("code", code)
        .add_attribute("sender", sender)
        .add_attribute("amount", amount.to_string())
        .add_attribute("memo", memo)
}

#[track_caller]
fn assert_event(received: &[Event], expected: &Event) {
    let found = received.iter().any(|ev| {
        expected.ty == ev.ty
            && expected
                .attributes
                .iter()
                .all(|at| ev.attributes.contains(at))
    });

    assert!(
        found,
        "Expected to find an event {:?}, but receiveed: {:?}",
        expected, received
    );
}

#[test]
fn redeem() {
    let mut suite = suite::Config::new()
        .with_member("member", 2000, 10)
        .init()
        .unwrap();

    let member = suite.members[0].clone();

    // member obviously can redeem funds
    let resp = suite
        .redeem(&member, 1000, "redeem-code-1", None, "First redeem")
        .unwrap();

    assert_event(
        &resp.events,
        &redeem_event("redeem-code-1", &member.to_string(), 1000, "First redeem"),
    );
    assert!(
        resp.events.iter().any(|ev| ev.ty == "wasm-redeem"),
        "No redeem event in response: {:?}",
        resp
    );
    assert_eq!(suite.balance(&member).unwrap(), 1000);
    assert_eq!(suite.total_supply().unwrap(), 1000);

    // members still can redeem after he is removed from whitelist
    suite.remove_member(&member).unwrap();
    let resp = suite
        .redeem(
            &member,
            500,
            "redeem-code-2",
            "receiver".to_owned(),
            "Second redeem",
        )
        .unwrap();

    assert_event(
        &resp.events,
        &redeem_event("redeem-code-2", "receiver", 500, "Second redeem"),
    );
    assert_eq!(suite.balance(&member).unwrap(), 500);
    assert_eq!(suite.total_supply().unwrap(), 500);
}
