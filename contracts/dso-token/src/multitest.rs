mod suite;

use cosmwasm_std::{Addr, Uint128};
use cw20::{Cw20ReceiveMsg, TokenInfoResponse};

use crate::msg::{IsWhitelistedResponse, QueryMsg, WhitelistResponse};

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
    let mut suite = suite::Config::new()
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
    let mut suite = suite::Config::new()
        .with_member("member", 1000, 10)
        .init()
        .unwrap();
    let member = suite.members[0].clone();

    // Instantiate receiver contract
    let receiver = suite::ReceiverContract::init(&mut suite.app, suite.owner.clone()).unwrap();

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
    let mut suite = suite::Config::new()
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
    let mut suite = suite::Config::new()
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
    let mut suite = suite::Config::new()
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
    let mut suite = suite::Config::new()
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
