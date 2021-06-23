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

You will also need cw20-base token contract from `cosmwasm-plus`:

* [`cw20-base` (v0.6.1)](https://github.com/CosmWasm/cosmwasm-plus/releases/download/v0.6.1/cw20_base.wasm)

Once you have these, upload all three codes and record the code ids. I will call them: `PAIR_ID`, `FACTORY_ID`,
`TOKEN_ID`.

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

