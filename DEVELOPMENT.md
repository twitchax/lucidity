# lucidity development

## Publish to Cargo

Make sure `rtz` references the correct versions of `rtz-core` and `rtz-build` in `Cargo.toml`.

```bash
$ cargo publish -p lucidity-core
$ cargo publish -p lucidity-macros
$ cargo publish -p lucidity
```