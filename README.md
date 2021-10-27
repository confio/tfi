# Tgrade RegFi

Components to build out Regulated DeFi for [Tgrade blockchain](https://tgrade.finance)

Current state:

* AMM (ported from `terraswap`)
* Whitelisted Token

## Contracts

| Name                                               | Description                                  |
| -------------------------------------------------- | -------------------------------------------- |
| [`tfi_factory`](contracts/tfi-factory) |             Factory Contract to create new trading pair             |
| [`tfi_pair`](contracts/tfi-pair)       |            Trading Pair that can swap two tokens             |
| [`trusted-token`](contracts/trusted-token)   |       CW20 Token that limits all holders to a whitelist               |

## Running this contract

You will need Rust 1.51+ with wasm32-unknown-unknown target installed.

You can run unit tests on this on each contracts directory via :

```
cargo unit-test
cargo integration-test
```

Or for a production-ready (compressed) build, run the following from the repository root:

```
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.12.1
```

The optimized contracts are generated in the artifacts/ directory.
