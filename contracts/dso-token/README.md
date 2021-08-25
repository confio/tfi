# DSO Token

This is a cw20-based token with whitelisting, to be used in the context of a DSO.

It can be used to provide one side of a trading pair in an AMM setting.

Only whitelisted users will be able to add liquidity to the token, and trade it.

## Differences between standard cw20 and dso-token

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

## New messages

### Reedem

```json
"reedem": {
  "amount": "1000",
  "code": "reedem-code",
  "sender": "original-requester",
  "memo": "Meta information"
}
```

Executing reedem on this contract effectively burns owned tokens. It is intended
to reedem tokens to provider as part of offchain transaction, typically when he
covers burned token value in other commodity.

`code` field is a value agreed with token provider to perform reedem with, to
allow him to easly identify reedem operation. Any code can be used only for single
reedem operation.

`sender` is account which requested reedem, if it is not the same who executed it.
It is optional, and message sender is used if none is provided.

`memo` is free text field where extra metadata or just message can be embeded.

When `Reedem` operation is completed, infomation about it is stored in contract
state, so it can be later queried by token provider. Also custom event is send
to blockchain:

```json
"reedem": {
  "code": "reedem-code",
  "sender": "addr-performing-reedem",
  "amount": "1000",
  "memo": "Meta information"
}
```

To finalize off-chain reedem operation, token provider might either subscribe on
`reedem` event, or periodically query for reedems.

### RemoveReedems

```json
"remove_reedems": {
  "codes": [
    "reedem-code1",
    "reedem-code2"
  ]
}
```

Removes stored reedems information related to provided reedem codes. Only minter
is allowed to do that.

### CleanReedems

```json
"clean_reedems" : {}
```

Removes all stored reedems information. Only minter is allowed to do that.

## New queries

### Query for single reedem

```json
"redeem": {
  "code": "reedem-code"
}
```

Returns:

```json
"ReedemResponse": {
  "reedem": {
    "sender": "addr-performed-reedem",
    "amount": "1000",
    "memo": "Meta information",
    "timestamp": 1629292639
  }
}
```

Queries for single reedem information. Besides of information provided by
`reedem` event, `timestamp` field is added to give an idea when reedem took place.

Response may be empty, not containing `reedem` field, which means, that not
reedem was performed with this code.

### Query for all reedems

```json
"all_reedems": {
  "start_after": "last-read-reedem",
  "limit": 10
}
```

Returns:

```json
"AllReedemsResponse": {
  "reedems": [
    {
      "code": "reedem-code",
      "sender": "addr-performed-reedem",
      "amount": "1000",
      "memo": "Meta information",
      "timestamp": 1629292639
    }
  ]
}
```

Queries for multiple reedems information. `reedems` field may contains multiple
entries, but up to `limit`. If `limit` is not provided, it is still possible,
that not all reedems are returned, it may be internal cap on items returned. To
ensure that is not a case, additional query with `start_after` set as last
retuned reedem code. If optional `start_after` is provided, then only items after
this item would be returned.
