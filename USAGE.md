# Contract Usage

This is a brief walk-through on how to deploy and use the contracts.
The target audience is a JS developer building a frontend client.

## Compatibility

This code is working with CosmWasm 0.14 and compatible with any wasmd 0.16 or 0.17 testnet.

## Upload code.

First, you need to download all the contracts to be deployed. Find the
[most recent tag of `tfi`](https://github.com/confio/tfi/tags), and click
on it to find artifacts. You will want to download:

* [`tfi_pair.wasm` (v0.0.3)](https://github.com/confio/tfi/releases/download/v0.0.3/tfi_pair.wasm)
* [`tfi_factory.wasm` (v0.0.3)](https://github.com/confio/tfi/releases/download/v0.0.3/tfi_factory.wasm)

You will also need cw20-base token contract from `cosmwasm-plus` (v0.6.2+):

* [`cw20-base` (v0.6.2)](https://github.com/CosmWasm/cosmwasm-plus/releases/download/v0.6.2/cw20_base.wasm)

Once you have these, upload all three codes and record the code ids. I will call them: `PAIR_ID`, `FACTORY_ID`,
`TOKEN_ID`.

Note that you must use `cw20-base` (or similar) for the token in `TOKEN_ID`, as the 
[`InstantiateMsg` is fixed](https://github.com/confio/tfi/blob/main/packages/tfi/src/token.rs#L7-L15).
This is used for *liquidity tokens*. You can use any custom (whitelist / trusted) token contract for the trading pairs.

## Instantiate the Factory

Each pair of tokens we wish to trade is controlled by a `tfi_pair` contract. However, the `tfi_factory` is responsible
for creating these pairs, and then serves as a central repository to list all available pairs.

You can see the [factory messages in Rust](https://github.com/confio/tfi/blob/main/packages/tfi/src/factory.rs)
or as [JSON Schemas](https://github.com/confio/tfi/tree/main/contracts/tfi-factory/schema)

To instantiate it, you want to pass this to instantiate `FACTORY_ID`:

```json
{
  "pair_code_id": "$PAIR_ID",
  "token_code_id": "$TOKEN_ID"
}
```

You only need one instance of the factory contract on a system. We can hardcode that in any client (as config).
It will create all pairs and list them all.

## Create some tokens to trade

You can use a native token (from faucet) for one side.

Create a cw20-compatible token (with the contract of your choice) for the other side.

Make sure you have an account with plenty of both. Use those names below instead of the placeholder I provide.

## Create a trading Pair

To create a new trading pair, execute the following on the `FACTORY_ADDR` created above:

```json
{
  "create_pair": {
    "asset_infos": [
      {"native":  "utgd"},
      {"token":  "wasm1hmdudppzceg27qsuq707tjg8rkgj7g5heszmrw"}
    ]
  }
}
```

Both sides can be native tokens or cw20 tokens as you choose. (Both native, both cw20, or a mix).

This will create a new contract at `PAIR_ADDR` for this token pair. To discover this contract later, you can
query it directly:

```json
{
  "pair": {
    "asset_infos": [
      {"native":  "utgd"},
      {"token":  "wasm1hmdudppzceg27qsuq707tjg8rkgj7g5heszmrw"}
    ]
  }
}
```

which returns something like this, listing both the `contract_addr` to use for swapping, and the `liquidity_token`
(`cw20-base` contract address that is issued to members of the liquidity pool):

```json
{
  "asset_infos": [
    {"native":  "utgd"},
    {"token":  "wasm1hmdudppzceg27qsuq707tjg8rkgj7g5heszmrw"}
  ],
  "contract_addr": "wasm1df43m7ag36twfxm998yakxp4dw74gvludpsxx0",
  "liquidity_token": "wasm1c9489dfpgpual8gwpd85xfc2tnwl6qsx8d0j80"
}
```

or list all pairs (with typical pagination):

```json
{
  "pairs": {
    "limit": 20,
    "start_after": [
      {"native":  "uatom"},
      {"native":  "utgd"}
    ]
  }
}
```

which returns a list of all pairs to paginate through:

```json
{
  "pairs": [
    {
      "asset_infos": [
        {"native":  "utgd"},
        {"token":  "wasm1hmdudppzceg27qsuq707tjg8rkgj7g5heszmrw"}
      ],
      "contract_addr": "wasm1df43m7ag36twfxm998yakxp4dw74gvludpsxx0",
      "liquidity_token": "wasm1c9489dfpgpual8gwpd85xfc2tnwl6qsx8d0j80"
    }
  ]
}
```

## Use AMM for one pair

Now that we have an open pair at `PAIR_ADDR`, we want to fill it with liquidity and do some swaps.

But first, do some queries to ensure you can observe any changes:

```json
{"pair": {}}
```

```json
{"pool": {}}
```

In this case, we want to seed the contract with 25000000 `utgd` and 52000000 `wasm1hmdudppzceg27qsuq707tjg8rkgj7g5heszmrw` (`ASSET_ID`).
The native tokens (`utgd`) can be sent directly with the request, but the cw20 tokens must be previously authorized.

On `ASSET_ID` allow the tokens to be moved to the AMM:

```json
{
  "increase_allowance": {
    "spender": "$PAIR_ID",
    "amount": "52000000"
  }
}
```

Then on `PAIR_ID`, fill it with liquidity, like this (slippage_tolerance is optional). Make sure you send the 25000000 `utgd`
along with the `ExecuteMsg`:

```json
{
  "provide_liquidity": {
    "assets": [
      {
        "amount": "25000000",
        "info": {
          "native": "utgd"
        }
      },
      {
        "amount": "52000000",
        "info": {
          "token": "wasm1hmdudppzceg27qsuq707tjg8rkgj7g5heszmrw"
        }
      }
    ],
    "slippage_tolerance": "0.12"
  }
}
```

This should provide some initial tokens in the pool, and you can query `{"pool":{}}` to see this.
It will also issue some `$liquidity_token` tokens to the sender to represent their stake in the pool,
which they can later withdraw.

Now, let's see what we can swap. You can test it out with a query. How about 2TGD (2000000 `utgd`)?

```json
{
  "simulation": {
    "offer_asset": {
      "amount": "2000000",
      "info": {
        "native": "utgd"
      }
    }
  }
}
```

Happy with that? Send that 2000000 `utgd` along with an `ExecuteMsg` that looks like:

```json
{
  "swap": {
    "offer_asset": {
      "amount": "2000000",
      "info": {
        "native": "utgd"
      }
    },
    "belief_price": "1.75",
    "max_spread": "0.08"
  }
}
```

(This will likely fail with these values and can be adjusted or omitted. 
Both `belief_price` and `max_spread` are optional, but nice to pass in for good UX)

If you want to swap from the cw20 token side, you need to send the cw20 tokens along with a message:

```json
{
  "swap": {
    "max_spread": "0.25"
  }
}
```

This can then be base64-encoded to `eyJzd2FwIjp7Im1heF9zcHJlYWQiOiIwLjI1In19Cg==`. You can generate this with
`echo '{"swap":{"max_spread":"0.25"}}' | base64`

And finally, we send a message to the cw20 token used on one side of the swap (`ASSET_ID`) to send tokens along
with this request to the AMM. The final result will work like sending the native tokens directly:

```json
{
  "send": {
    "contract": "$PAIR_ID",
    "amount": "5000000",
    "msg": "eyJzd2FwIjp7Im1heF9zcHJlYWQiOiIwLjI1In19Cg"
  }
}
```
