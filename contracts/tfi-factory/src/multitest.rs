mod suite;

use anyhow::Error;

/// Compares if error is as expected
///
/// Unfortunatelly, error types information is lost, as in multitest every error is just converted
/// to its string representation. To solve this issue and still be able to reasonably test returned
/// error, but to avoid maintaining error string validation, errors are passed strongly typed, but
/// verified on their representation level. Additionally when error doesn't match, the actuall
/// error is printed in debug form so additional `anyhow` information is displayed.
#[track_caller]
fn assert_error(err: Error, expected: impl ToString + std::fmt::Debug) {
    assert_eq!(
        err.to_string(),
        expected.to_string(),
        "received error {:?} while expected {:?}",
        err,
        expected
    );
}

/// The simplest full stack success flow test scenario
///
/// All actors are whitelisted. Single pair is created by factory, end then it is whitelisted. Then
/// liquidity is provided, swaps in both directions are performed, and liquidity is whitedrawn.
///
/// No state checks are performed, only the fact that all operations successes. All logic is
/// covered by UTs, and tfi-pair integration tests.
#[test]
fn everyone_whitelisted() {
    let mut suite = suite::Config::new()
        .with_actor("liquidity-provider", 2000, 6000, true)
        .with_actor("trader", 1000, 1000, true)
        .init()
        .unwrap();

    let (cash, lp, trader) = (
        suite.cash.clone(),
        suite.actors[0].clone(),
        suite.actors[1].clone(),
    );

    let (pair, lt) = suite
        .create_pair([suite.btc(), suite.cash()], None)
        .unwrap();

    suite
        .add_member(&pair)
        .unwrap()
        .increase_allowance(&cash.addr(), &lp, &pair, 6000)
        .unwrap()
        .provide_liquidity(&pair, &lp, 2000, 6000)
        .unwrap()
        .swap_btc(&pair, &trader, 1000)
        .unwrap()
        .swap_cash(&pair, &trader, 1000)
        .unwrap();

    let share = lt.balance(&suite.app, &lp).unwrap();

    suite
        .withdraw_liquidity(&pair, &lt.addr(), &lp, share.into())
        .unwrap();
}

/// Failure test showing up, that it is impossible to provide liquidity to pair if it is not part
/// of the whitelist
#[test]
fn pair_not_whitelisted() {
    let mut suite = suite::Config::new()
        .with_actor("liquidity-provider", 2000, 6000, true)
        .init()
        .unwrap();

    let (cash, lp) = (suite.cash.clone(), suite.actors[0].clone());

    let (pair, _) = suite
        .create_pair([suite.btc(), suite.cash()], None)
        .unwrap();

    let err = suite
        .increase_allowance(&cash.addr(), &lp, &pair, 6000)
        .unwrap()
        .provide_liquidity(&pair, &lp, 2000, 6000)
        .unwrap_err();

    assert_error(err, trusted_token::error::ContractError::Unauthorized {});
}

/// Failure test showing up, that it is impossible to provide liquidity nor swap with pair by
/// non-whitelisted actors
#[test]
fn actors_not_whitelisted() {
    let mut suite = suite::Config::new()
        .with_actor("member-liquidity-provider", 2000, 6000, true)
        .with_actor("liquidity-provider", 2000, 6000, false)
        .with_actor("trader", 1000, 1000, false)
        .init()
        .unwrap();

    let (cash, member_lp, lp, trader) = (
        suite.cash.clone(),
        suite.actors[0].clone(),
        suite.actors[1].clone(),
        suite.actors[2].clone(),
    );

    // Preparation
    let (pair, _) = suite
        .create_pair([suite.btc(), suite.cash()], None)
        .unwrap();

    suite
        .add_member(&pair)
        .unwrap()
        .increase_allowance(&cash.addr(), &member_lp, &pair, 6000)
        .unwrap()
        .provide_liquidity(&pair, &member_lp, 2000, 6000)
        .unwrap();

    // Non-member liquidity provider cannot even increase allowance
    let err = suite
        .increase_allowance(&cash.addr(), &lp, &pair, 6000)
        .unwrap_err();
    assert_error(err, trusted_token::error::ContractError::Unauthorized {});

    // As non-member liquidity provider is not allowed to provide liquidity
    let err = suite.provide_liquidity(&pair, &lp, 2000, 6000).unwrap_err();
    assert_error(err, trusted_token::error::ContractError::Unauthorized {});

    // Even if lp is later added, he would need to increase allowance first, as previous attempt
    // failed
    let err = suite
        .add_member(&lp)
        .unwrap()
        .provide_liquidity(&pair, &lp, 2000, 6000)
        .unwrap_err();
    assert_error(err, cw20_base::ContractError::NoAllowance {});

    // Non whitelisted members has no swap rights
    let err = suite.swap_btc(&pair, &trader, 1000).unwrap_err();
    assert_error(err, trusted_token::error::ContractError::Unauthorized {});

    let err = suite.swap_cash(&pair, &trader, 1000).unwrap_err();
    assert_error(err, trusted_token::error::ContractError::Unauthorized {});
}
