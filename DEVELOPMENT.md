# lucidity development

## Publish to Cargo

Make sure `lucidity` references the correct versions of `lucidity-core` and `lucidity-macros` in `Cargo.toml`.

```bash
$ cargo publish -p lucidity-core
$ cargo publish -p lucidity-macros
$ cargo publish -p lucidity
```