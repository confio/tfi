# Trusted Token

This is a cw20-based token with whitelisting, to be used in the context of a Trusted Circle.

It can be used to provide one side of a trading pair in an AMM setting.

Only whitelisted users will be able to add liquidity to the token, and trade it.

## Differences between standard cw20 and trusted-token

### Instantiation

Instantiate message consist additional `whitelist_group`:

```rust
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<Cw20Coin>,
    pub mint: Option<MinterResponse>,
    pub marketing: Option<InstantiateMarketingInfo>,
    pub whitelist_group: String,
}
```

New field is the address of cw4 group contract. Only addresses being members of
this group would be able to trade this token.

### Execution

We override the `execute` method and check that all addresses (sender, recipient, owner if sending on someone else's behalf)
are members of the whitelist contract before dispatching to the standard `cw20-base` action.  This looks like:

```rust
pub fn execute() {
  let res = match msg {
    ExecuteMsg::Transfer { recipient, amount } => {
      verify_sender_and_addresses_on_whitelist(deps.as_ref(), &info.sender, & [&recipient])?;
      cw20_base::contract::execute_transfer(deps, env, info, recipient, amount)?
    }
    // other variants....
  };
  // rest of code...
}
```

The key logic then looks like this:

```rust
pub(crate) fn verify_sender_and_addresses_on_whitelist(
    deps: Deps,
    sender: &Addr,
    addresses: &[&str],
) -> Result<(), ContractError> {
    let whitelist: Cw4Contract = WHITELIST.load(deps.storage)?;
    if whitelist.is_member(&deps.querier, sender)?.is_none() {
        return Err(ContractError::Unauthorized {});
    }
    for address in addresses {
        let validated_address = deps.api.addr_validate(address)?;
        if whitelist
            .is_member(&deps.querier, &validated_address)?
            .is_none()
        {
            return Err(ContractError::Unauthorized {});
        }
    }
    Ok(())
}
```

Note that it just checks if the member is present in the group contract, it is unimportant what weight it has.
This means that even 0 weight (which will not allow it to vote in voting contracts) is sufficient to pass the whitelist.
It must be fully removed from the group contract to no longer pass the whitelisting check.

## New messages

### Redeem

```json
{
  "redeem": {
    "amount": "1000",
    "code": "redeem-code",
    "sender": "original-requester",
    "memo": "Meta information"
  }
}
```

Executing redeem on this contract effectively burns owned tokens. It is intended
to redeem tokens to provider as part of offchain transaction, typically when he
covers burned token value in other commodity.

`code` field is a value agreed with token provider to perform redeem with, to
allow him to easly identify redeem operation. Any code can be used only for single
redeem operation.

`sender` is account which requested redeem, if it is not the same who executed it.
It is optional, and message sender is used if none is provided.

`memo` is free text field where extra metadata or just message can be embeded.

When `Redeem` operation is completed, infomation about it is stored in contract
state, so it can be later queried by token provider. Also custom event is send
to blockchain:

```json
{
  "redeem": {
    "code": "redeem-code",
    "sender": "addr-performing-redeem",
    "amount": "1000",
    "memo": "Meta information"
  }
}
```

To finalize off-chain redeem operation, token provider might either subscribe on
`redeem` event, or periodically query for redeems.

### RemoveRedeems

```json
{
  "remove_redeems": {
    "codes": [
      "redeem-code1",
      "redeem-code2"
    ]
  }
}
```

Removes stored redeems information related to provided redeem codes. Only minter
is allowed to do that.

### CleanRedeems

```json
{ "clean_redeems" : {} }
```

Removes all stored redeems information. Only minter is allowed to do that.

## New queries

### Query for single redeem

```json
{
  "redeem": {
    "code": "redeem-code"
  }
}
```

Returns:

```json
{
  "RedeemResponse": {
    "redeem": {
      "sender": "addr-performed-redeem",
      "amount": "1000",
      "memo": "Meta information",
      "timestamp": 1629292639
    }
  }
}
```

Queries for single redeem information. Besides of information provided by
`redeem` event, `timestamp` field is added to give an idea when redeem took place.

Response may be empty, not containing `redeem` field, which means, that not
redeem was performed with this code.

### Query for all redeems

```json
{
  "all_redeems": {
    "start_after": "last-read-redeem",
    "limit": 10
  }
}
```

Returns:

```json
{
  "AllRedeemsResponse": {
    "redeems": [
      {
        "code": "redeem-code",
        "sender": "addr-performed-redeem",
        "amount": "1000",
        "memo": "Meta information",
        "timestamp": 1629292639
      }
    ]
  }
}
```

Queries for multiple redeems information. `redeems` field may contains multiple
entries, but up to `limit`. If `limit` is not provided, it is still possible,
that not all redeems are returned, it may be internal cap on items returned. To
ensure that is not a case, additional query with `start_after` set as last
retuned redeem code. If optional `start_after` is provided, then only items after
this item would be returned.