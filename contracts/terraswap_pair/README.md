# TerraSwap Pair Contract

## Handlers
### Initialize
This is mainly used from terraswap factory contract to create new terraswap pair. It initialize all swap created parameters which can be updated later with owner key. 

It creates liquidity token contract as init response, and execute init hook to register created liquidity token contract to self. 

```rust
{
    /// Contract owner who can update configs
    pub owner: HumanAddr,
    /// Inactive commission collector
    pub commission_collector: HumanAddr,
    /// Asset infos
    pub asset_infos: [AssetInfo; 2],
    /// Commission rate for active liquidity provider
    pub lp_commission: Decimal,
    /// Commission rate for owner controlled commission
    pub owner_commission: Decimal,
    /// Token contract code id for initialization
    pub token_code_id: u64,
    /// Hook for post initalization
    pub init_hook: Option<InitHook>,
}
```

### UpdateConfig
    
The market contract owner can update commission configuration with `update_config` msg.

```json
{
    "update_config": 
    {
        "owner": Option<HumanAddr>,
        "lp_commission": Option<Decimal>,
        "owner_commission": Option<Decimal>,
    }
}
```

### Liquidity Provider

The contract has two types of pool, the one is collateral and the other is asset pool. A user can provide liquidity to each pool by sending `provide_liquidity` msgs and also can withdraw with `withdraw_liquidity` msgs. 

Whenever liquidity is deposited into a pool, special tokens known as liquidity tokens are minted to the provider’s address, in proportion to how much liquidity they contributed to the pool. These tokens are a representation of a liquidity provider’s contribution to a pool. Whenever a trade occurs, the `lp_commission%` of fee is distributed pro-rata to all LPs in the pool at the moment of the trade. To receive the underlying liquidity back, plus commission fees that were accrued while their liquidity was locked, LPs must burn their liquidity tokens.

When providing liquidity from a smart contract, the most important thing to keep in mind is that tokens deposited into a pool at any rate other than the current oracle price ratio are vulnerable to being arbitraged. As an example, if the ratio of x:y in a pair is 10:2 (i.e. the price is 5), and someone naively adds liquidity at 5:2 (a price of 2.5), the contract will simply accept all tokens (changing the price to 3.75 and opening up the market to arbitrage), but only issue pool tokens entitling the sender to the amount of assets sent at the proper ratio, in this case 5:1. To avoid donating to arbitrageurs, it is imperative to add liquidity at the current price. Luckily, it’s easy to ensure that this condition is met!

> Note before executing the `provide_liqudity` operation, a user must allow the contract to use the liquidity amount of asset in the token contract.


Request Format
* Provide Liquidity
  
    ```json
    { 
        "provide_liquidity": { 
            "coins": [{
                "denom": "APPL", 
                "amount": "1000000"
            }]
        } 
    }
    ```
* Withdraw Liquidity
  
    ```json
    { 
        "withdraw_liquidity": { 
            "amount": "1000000" 
        } 
    }
    ```

### Swap

Any user can swap an asset by sending `swap` or invoking `send` msg to token contract with `swap` hook message.

* Native Token => Token
  
    ```json
    { 
        "swap ": { 
            "offer_asset": {
                "info": {
                    "native_token": {
                        "denom": String 
                    }
                }, 
                "amount": Uint128
            }, 
            "max_spread": Option<Decimal> 
        } 
    }
    ```

* Token => Native Token

    **Must be sent to token contract**
    ```json
    { 
        "send": {
            "contract": HumanAddr, 
            "amount": Uint128, 
            "msg": Binary({
                "swap": {
                    "max_spread": Option<Decimal>
                }
            })
        }
    }
    ```

#### Swap Spread

The spread is determined with following uniswap mechanism:

```rust
// -max_minus_spread < spread < max_spread
// minus_spread means discount rate.
// Ensure `asset pool * collateral pool = constant product`
let cp = Uint128(offer_pool.u128() * ask_pool.u128());
let return_amount = offer_amount * exchange_rate;
let return_amount = (ask_pool - cp.multiply_ratio(1u128, offer_pool + offer_amount))?;


// calculate spread & commission
let spread_amount: Uint128 =
    (offer_amount * Decimal::from_ratio(ask_pool, offer_pool) - return_amount)?;
let lp_commission: Uint128 = return_amount * config.lp_commission;
let owner_commission: Uint128 = return_amount * config.owner_commission;

// commission will be absorbed to pool
let return_amount: Uint128 =
    (return_amount - (lp_commission + owner_commission)).unwrap();
```

#### Commission
The `lp_commission` remains in the swap pool, causing a permanent increase in the constant product K. The value of this permanently increased pool goes to all LPs.

The `owner_commssion` is transferred to pre-defined commission collector address.
