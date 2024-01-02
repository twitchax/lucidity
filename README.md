[![Build and Test](https://github.com/twitchax/lucidity/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/lucidity/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/lucidity/branch/main/graph/badge.svg?token=35MZN0YFZF)](https://codecov.io/gh/twitchax/lucidity)
[![Version](https://img.shields.io/crates/v/lucidity.svg)](https://crates.io/crates/lucidity)
[![Crates.io](https://img.shields.io/crates/d/lucidity?label=crate)](https://crates.io/crates/lucidity)
[![GitHub all releases](https://img.shields.io/github/downloads/twitchax/lucidity/total?label=binary)](https://github.com/twitchax/lucidity/releases)
[![Documentation](https://docs.rs/lucidity/badge.svg)](https://docs.rs/lucidity)
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