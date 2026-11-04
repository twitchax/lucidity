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
$ cargo run --target wasm32-wasip1 --example <name>
```

`cargo run` (a plain `lunatic run`) is enough for local jobs.  Remote jobs need the code to run inside a `lunatic node`
registered with a control node that has peers (see `lucidity/examples/pythagorean.rs` for the commands).

## Test

```bash
$ cargo test -p lucidity-macros   # macro expansion
$ cargo test -p lucidity-e2e      # the examples on a real local cluster (needs the runtime built, or `LUNATIC_BIN`)
$ cargo test --target wasm32-wasip1 -p lucidity --features fly --lib   # the fly module, under the runtime
```

## Publish to Cargo

Make sure `lucidity` references the correct versions of `lucidity-core` and `lucidity-macros` in `Cargo.toml`.  The `lunatic` path dependency needs a published crate (and `version`) before publishing.

```bash
$ cargo publish -p lucidity-core -p lucidity-macros -p lucidity
```

Cargo publishes the three in dependency order, waiting for each to reach the index.