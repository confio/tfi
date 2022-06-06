# Trusted Token

This is a cw20-based token with whitelisting, to be used in the context of a Trusted Circle.

It can be used to provide one side of a trading pair in an AMM setting.

Only whitelisted users will be able to add liquidity to the token, and trade it.

## Differences between standard cw20 and trusted-token

### Instantiation

Instantiate message contains an additional `whitelist_group`:

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

This new field is the address of tg4 group contract. Only addresses that are members of
this group would be able to trade this token.

### Execution

We override the `execute` method and check that all addresses (sender, recipient, and owner if sending on someone else's behalf)
are members of the whitelist contract, before dispatching to the standard `cw20-base` action.  This looks like:

```rust
pub fn execute() {
  let res = match msg {
    ExecuteMsg::Transfer { recipient, amount } => {
      verify_sender_and_addresses_on_whitelist(deps.as_ref(), &info.sender, & [&recipient])?;
      cw20_base::contract::execute_transfer(deps, env, info, recipient, amount)?
    }
    // other variants....
  };
  // rest of the code...
}
```

The key logic then looks like this:

```rust
pub(crate) fn verify_sender_and_addresses_on_whitelist(
    deps: Deps,
    sender: &Addr,
    addresses: &[&str],
) -> Result<(), ContractError> {
    let whitelist: Tg4Contract = WHITELIST.load(deps.storage)?;
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

Note that this just checks if the member is present in the group contract; it is unimportant what weight it has.
This means that even 0 weight members (which would not allow them to vote in voting contracts) can pass the whitelist.
A member must be fully removed from the group contract to no longer pass the whitelisting check.

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

Executing redeem on this contract effectively burns the owned tokens. It is intended
to redeem tokens to the provider as part of off-chain transaction; typically when he
covers the burned token value in another commodity.

The `code` field is a value agreed with token provider to perform redeem with, to
allow him to easily identify the redeem operation. A code can only be used for a single
redeem operation.

`sender` is the account who requested `Redeem`, if it is not the same who executed it.
It is optional, and the message sender is used if none is provided.

`memo` is a free text field where an extra metadata or message can be embedded.

When the `Redeem` operation is completed, information about it is stored in contract
state, so that it can be later queried by the token provider. Also, a custom event is sent
to the blockchain:

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

To finalize an off-chain redeem operation, the token provider might either subscribe to
the `redeem` event, or periodically query for redeems.

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

Removes stored redeem information related to the provided redeem codes. Only the minter
is allowed to do that.

### CleanRedeems

```json
{ "clean_redeems" : {} }
```

Removes all stored redeems information. Only the minter is allowed to do that.

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

Queries for single redeem information. Besides information provided by the
`redeem` event, a `timestamp` field is added to give an idea of when the redeem took place.

The response may be empty, not containing a `redeem` field, which means that no
redemption was performed with this code.

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

Queries for multiple redeems information. The `redeems` field may contain multiple
entries, up to `limit`. If `limit` is not provided, it is still possible
that not all redeems are returned, as there may be an internal cap on the
numer of returned items per query. To ensure that that is not a case, an additional query
with `start_after` set as the last returned redeem code can be performed.
If an optional `start_after` is provided, then only items after
this item will be returned.
