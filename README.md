# Tgrade RegFi

Components to build out Regulated DeFi for [Tgrade blockchain](https://tgrade.finance)

* AMM (ported from `terraswap`)
* Whitelisted Token

## Contracts

TODO

| Name                                               | Description                                  |
| -------------------------------------------------- | -------------------------------------------- |
| [`tfi_factory`](contracts/tfi_factory) |                                              |
| [`tfi_pair`](contracts/tfi_pair)       |                                              |
| [`tfi_router`](contracts/tfi_router)   |                                              |

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
  cosmwasm/workspace-optimizer:0.11.2
```

The optimized contracts are generated in the artifacts/ directory.
