# lucidity development

## Layout

`lucidity` builds against forks of the lunatic SDK and runtime, checked out as siblings:

```
<parent>/
├── lucidity     # this repo
├── lunatic-rs   # https://github.com/twitchax/lunatic-rs (path dependency, see `Cargo.toml`)
└── lunatic      # https://github.com/twitchax/lunatic (cargo runner, see `.cargo/config.toml`)
```

Build the runtime once (and after any runtime change) before running guest code:

```bash
$ (cd ../lunatic && cargo build --release)
$ cargo build --target wasm32-wasip1 --example <name>
```

Jobs spawn through the distributed API, so guest code runs inside a `lunatic node` registered with a control node
(see `lucidity/examples/pythagorean.rs` for the commands), not under `cargo run` / `lunatic run`.

## Publish to Cargo

Make sure `lucidity` references the correct versions of `lucidity-core` and `lucidity-macros` in `Cargo.toml`.  The `lunatic` path dependency needs a published crate (and `version`) before publishing.

```bash
$ cargo publish -p lucidity-core
$ cargo publish -p lucidity-macros
$ cargo publish -p lucidity
```