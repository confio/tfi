# tfi Router <!-- omit in toc -->

The Router Contract contains the logic to facilitate multi-hop swap operations tfi-pairs

### Operations Assertion
The contract will check whether the resulting token is swapped into one token.

### Example

Swap KRT => UST => mABNB
```
{
   "execute_swap_operations":{
      "operations":[
         {
            "native_swap":{
               "offer_denom":"ukrw",
               "ask_denom":"uusd"
            }
         },
         {
            "terra_swap":{
               "offer_asset_info":{
                  "native_token":{
                     "denom":"uusd"
                  }
               },
               "ask_asset_info":{
                  "token":{
                     "contract_addr":"terra1avryzxnsn2denq7p2d7ukm6nkck9s0rz2llgnc"
                  }
               }
            }
         }
      ],
      "minimum_receive":"88000"
   }
}
```
