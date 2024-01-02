[![Build and Test](https://github.com/twitchax/rtz/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/rtz/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/rtz/branch/main/graph/badge.svg?token=35MZN0YFZF)](https://codecov.io/gh/twitchax/rtz)
[![Version](https://img.shields.io/crates/v/rtz.svg)](https://crates.io/crates/rtz)
[![Crates.io](https://img.shields.io/crates/d/rtz?label=crate)](https://crates.io/crates/rtz)
[![GitHub all releases](https://img.shields.io/github/downloads/twitchax/rtz/total?label=binary)](https://github.com/twitchax/rtz/releases)
[![npm](https://img.shields.io/npm/dt/rtzweb?label=npm)](https://www.npmjs.com/package/rtzweb)
[![Documentation](https://docs.rs/rtz/badge.svg)](https://docs.rs/rtz)
[![Rust](https://img.shields.io/badge/rust-nightly-blue.svg?maxAge=3600)](https://github.com/twitchax/rtz)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# lucidity

A distributed orchestrator built upon [lunatic]().

## Library Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
lucidity = "*" # choose a version
```

In your `.cargo/config.toml`:

```toml
[build]
target = "wasm32-wasi"

[target.wasm32-wasi]
runner = "lunatic run"
```

### Examples

## Feature Flags

## Test

```bash
cargo test
```

## License

MIT