# Changelog

## [Unreleased](https://github.com/confio/tfi/tree/HEAD)

[Full Changelog](https://github.com/confio/tfi/compare/v0.3.0...HEAD)

**Fixed bugs:**

- Change tfi-pair instance label to fixed string [\#69](https://github.com/confio/tfi/pull/69) ([maurolacy](https://github.com/maurolacy))

**Merged pull requests:**

- Add changelog and script to regenerate it [\#67](https://github.com/confio/tfi/pull/67) ([ueco-jb](https://github.com/ueco-jb))

## [v0.3.0](https://github.com/confio/tfi/tree/v0.3.0) (2022-02-17)

[Full Changelog](https://github.com/confio/tfi/compare/v0.2.2...v0.3.0)

**Closed issues:**

- Change `trusted-token`'s cw4 usage to tg4 [\#64](https://github.com/confio/tfi/issues/64)
- Better error message for slippage [\#61](https://github.com/confio/tfi/issues/61)
- tfi contracts use cw2::set\_contract\_version [\#59](https://github.com/confio/tfi/issues/59)

**Merged pull requests:**

- Update contract's version to 0.3.0 [\#66](https://github.com/confio/tfi/pull/66) ([ueco-jb](https://github.com/ueco-jb))
- Replace cw4 and cw4-group contracts with tg4 and tg4-group [\#65](https://github.com/confio/tfi/pull/65) ([ueco-jb](https://github.com/ueco-jb))
- tfi-pair: Better error messages for spread and slippage exceeding [\#63](https://github.com/confio/tfi/pull/63) ([hashedone](https://github.com/hashedone))
- Ci update rust [\#62](https://github.com/confio/tfi/pull/62) ([maurolacy](https://github.com/maurolacy))
- Use cw2::set\_contract\_version in tfi-factory and tfi-pair [\#60](https://github.com/confio/tfi/pull/60) ([ueco-jb](https://github.com/ueco-jb))

## [v0.2.2](https://github.com/confio/tfi/tree/v0.2.2) (2021-10-29)

[Full Changelog](https://github.com/confio/tfi/compare/v0.1.1...v0.2.2)

**Closed issues:**

- Update license [\#56](https://github.com/confio/tfi/issues/56)
- Rename dso-token to trusted-token [\#54](https://github.com/confio/tfi/issues/54)
- Update to cosmwasm 1.0-beta and cw-plus 0.10 [\#51](https://github.com/confio/tfi/issues/51)

**Merged pull requests:**

- Cleanup unused code [\#58](https://github.com/confio/tfi/pull/58) ([ethanfrey](https://github.com/ethanfrey))
- Update license [\#57](https://github.com/confio/tfi/pull/57) ([ethanfrey](https://github.com/ethanfrey))
- Rename dso-token to trusted-token [\#55](https://github.com/confio/tfi/pull/55) ([ethanfrey](https://github.com/ethanfrey))
- Update to cosmwasm 1.0-beta and cw-plus 0.10 [\#53](https://github.com/confio/tfi/pull/53) ([ueco-jb](https://github.com/ueco-jb))

## [v0.1.1](https://github.com/confio/tfi/tree/v0.1.1) (2021-10-14)

[Full Changelog](https://github.com/confio/tfi/compare/v0.1.0...v0.1.1)

**Closed issues:**

- Add event for new coins created [\#49](https://github.com/confio/tfi/issues/49)
- Upgrade mutlitest to 0.8.1 [\#47](https://github.com/confio/tfi/issues/47)
- Improve code/tests on whitelist [\#15](https://github.com/confio/tfi/issues/15)
- Extend whitelist token with "redeem" function as defined in DSO spec [\#12](https://github.com/confio/tfi/issues/12)
- Add full stack whitelist AMM test [\#9](https://github.com/confio/tfi/issues/9)

**Merged pull requests:**

- Add event for creating new coin in dso-token instantiate msg [\#52](https://github.com/confio/tfi/pull/52) ([ueco-jb](https://github.com/ueco-jb))
- Add trusted circle api [\#50](https://github.com/confio/tfi/pull/50) ([ethanfrey](https://github.com/ethanfrey))
- Update cw-multi-test to 0.8.1 [\#48](https://github.com/confio/tfi/pull/48) ([ueco-jb](https://github.com/ueco-jb))

## [v0.1.0](https://github.com/confio/tfi/tree/v0.1.0) (2021-08-26)

[Full Changelog](https://github.com/confio/tfi/compare/v0.1.0-rc...v0.1.0)

**Closed issues:**

- Improve code/tests on whitelist [\#15](https://github.com/confio/tfi/issues/15)
- Extend whitelist token with "redeem" function as defined in DSO spec [\#12](https://github.com/confio/tfi/issues/12)
- Add full stack whitelist AMM test [\#9](https://github.com/confio/tfi/issues/9)
- Expose commission in tfi-factory [\#35](https://github.com/confio/tfi/issues/35)
- Make tfi-pair commission configurable [\#34](https://github.com/confio/tfi/issues/34)
- Upgrade to CosmWasm 0.16 [\#32](https://github.com/confio/tfi/issues/32)
- Update to CosmWasm / CosmWasm-Plus final releases [\#31](https://github.com/confio/tfi/issues/31)
- Add a logo for the Issued tokens [\#27](https://github.com/confio/tfi/issues/27)
- Add multitest for tfi-factory / tf-pair [\#10](https://github.com/confio/tfi/issues/10)

**Merged pull requests:**

- Redeem implementation for dso-token [\#46](https://github.com/confio/tfi/pull/46) ([hashedone](https://github.com/hashedone))
- Demo unit test querier [\#45](https://github.com/confio/tfi/pull/45) ([ethanfrey](https://github.com/ethanfrey))
- Multitest veryfing full tfi-factory workflow [\#44](https://github.com/confio/tfi/pull/44) ([hashedone](https://github.com/hashedone))
- dso-token tests refactoring and covering whole validation functionality [\#43](https://github.com/confio/tfi/pull/43) ([hashedone](https://github.com/hashedone))

## [v0.1.0-rc](https://github.com/confio/tfi/tree/v0.1.0-rc) (2021-08-12)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.7...v0.1.0-rc)

**Closed issues:**

- Expose commission in tfi-factory [\#35](https://github.com/confio/tfi/issues/35)
- Make tfi-pair commission configurable [\#34](https://github.com/confio/tfi/issues/34)
- Upgrade to CosmWasm 0.16 [\#32](https://github.com/confio/tfi/issues/32)
- Update to CosmWasm / CosmWasm-Plus final releases [\#31](https://github.com/confio/tfi/issues/31)
- Add a logo for the Issued tokens [\#27](https://github.com/confio/tfi/issues/27)
- Remove all references to tax in tif-pair [\#25](https://github.com/confio/tfi/issues/25)
- Upgrade to cosmwasm 0.16 [\#17](https://github.com/confio/tfi/issues/17)
- Add multitest for tfi-factory / tf-pair [\#10](https://github.com/confio/tfi/issues/10)

**Merged pull requests:**

- Commission value validation [\#42](https://github.com/confio/tfi/pull/42) ([hashedone](https://github.com/hashedone))
- Commission configuration on tfi-factory [\#41](https://github.com/confio/tfi/pull/41) ([hashedone](https://github.com/hashedone))
- Make commission fully configurable on tfi-pair [\#40](https://github.com/confio/tfi/pull/40) ([hashedone](https://github.com/hashedone))
- Upgrade cosmwasm-plus dependencies to 0.8.0 [\#39](https://github.com/confio/tfi/pull/39) ([hashedone](https://github.com/hashedone))
- Expose cw20 logo and marketing info API on dso-token [\#38](https://github.com/confio/tfi/pull/38) ([hashedone](https://github.com/hashedone))
- Upgrade tfi dependencies to cosmwasm 0.16.0 and cosmwasm-plus 0.8.0-rc3 [\#36](https://github.com/confio/tfi/pull/36) ([hashedone](https://github.com/hashedone))
- Mutlitests for tfi pair [\#33](https://github.com/confio/tfi/pull/33) ([hashedone](https://github.com/hashedone))
- Update to cosmwasm 0.16.0-rc5 [\#30](https://github.com/confio/tfi/pull/30) ([ethanfrey](https://github.com/ethanfrey))
- Remove tax references [\#28](https://github.com/confio/tfi/pull/28) ([ethanfrey](https://github.com/ethanfrey))
- Added query for checking if address is blacklisted for dso-token [\#21](https://github.com/confio/tfi/pull/21) ([hashedone](https://github.com/hashedone))
- Add whitelist member query [\#20](https://github.com/confio/tfi/pull/20) ([ethanfrey](https://github.com/ethanfrey))
- tfi-factory: non-empty label when instantiating tfi-pair [\#19](https://github.com/confio/tfi/pull/19) ([ethanfrey](https://github.com/ethanfrey))
- Dso token contract [\#18](https://github.com/confio/tfi/pull/18) ([maurolacy](https://github.com/maurolacy))
- Fix clippy --tests warnings [\#16](https://github.com/confio/tfi/pull/16) ([ethanfrey](https://github.com/ethanfrey))
- Update to cosmwasm 0.15.0 [\#13](https://github.com/confio/tfi/pull/13) ([maurolacy](https://github.com/maurolacy))

## [v0.0.7](https://github.com/confio/tfi/tree/v0.0.7) (2021-07-22)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.6...v0.0.7)

**Merged pull requests:**

- Reproduce simulate issue [\#24](https://github.com/confio/tfi/pull/24) ([ethanfrey](https://github.com/ethanfrey))

## [v0.0.6](https://github.com/confio/tfi/tree/v0.0.6) (2021-07-22)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.5...v0.0.6)

**Merged pull requests:**

- Removed obsolete expects from contract [\#23](https://github.com/confio/tfi/pull/23) ([hashedone](https://github.com/hashedone))

## [v0.0.5](https://github.com/confio/tfi/tree/v0.0.5) (2021-07-21)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.4...v0.0.5)

**Merged pull requests:**

- Debug tfi factory [\#22](https://github.com/confio/tfi/pull/22) ([ethanfrey](https://github.com/ethanfrey))

## [v0.0.4](https://github.com/confio/tfi/tree/v0.0.4) (2021-07-19)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.3...v0.0.4)

**Closed issues:**

- Update to cosmwasm 0.15 / cosmwasm-plus 0.7 [\#11](https://github.com/confio/tfi/issues/11)
- Import cw20-whitelist contract [\#8](https://github.com/confio/tfi/issues/8)

## [v0.0.3](https://github.com/confio/tfi/tree/v0.0.3) (2021-06-23)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.2...v0.0.3)

**Merged pull requests:**

- Remove code [\#7](https://github.com/confio/tfi/pull/7) ([ethanfrey](https://github.com/ethanfrey))

## [v0.0.2](https://github.com/confio/tfi/tree/v0.0.2) (2021-06-23)

[Full Changelog](https://github.com/confio/tfi/compare/v0.0.1...v0.0.2)

**Merged pull requests:**

- Improve api [\#6](https://github.com/confio/tfi/pull/6) ([ethanfrey](https://github.com/ethanfrey))

## [v0.0.1](https://github.com/confio/tfi/tree/v0.0.1) (2021-06-23)

[Full Changelog](https://github.com/confio/tfi/compare/1ba9f1107fa449908cb9daa4a0409ee5dac93e0f...v0.0.1)

**Closed issues:**

- Set Up CI [\#2](https://github.com/confio/tfi/issues/2)

**Merged pull requests:**

- Simplify deps 2 [\#5](https://github.com/confio/tfi/pull/5) ([ethanfrey](https://github.com/ethanfrey))
- Setup CI [\#3](https://github.com/confio/tfi/pull/3) ([ethanfrey](https://github.com/ethanfrey))



\* *This Changelog was automatically generated by [github_changelog_generator](https://github.com/github-changelog-generator/github-changelog-generator)*
