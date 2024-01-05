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

## Motivation

Basically, `lunatic` by itself is a set of "low-level" runtime, syscalls, and language-wrappers.

However, the `Process` architecture is a low-ish-level primitive, and is a bit hard to use when you want your code to be more readable.  So, I am building a layer in Rust with proc macros that can make code that looks like this.

```rust
fn main() {
    let results = pythagorean_remote_fanout(vec![
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 8),
        (8, 9),
        (9, 10),
    ]);

    println!("result: {:#?}", results);
}

#[lucidity::job]
fn pythagorean(a: u32, b: u32) -> f32 {
    let num = ((square_remote_async(a).await_get() + square_remote_async(b).await_get()) as f32).sqrt();

    num
}

#[lucidity::job]
fn square(a: u32) -> u32 {
    let num = a * a;

    num
}
```

For each method you place the proc macro on, it generates a few others.

* `{name}_local`, when called, spawns the function in a _node local_ process, and blocks the calling process.
* `{name}_remote`, when called, spawns the function in a process on a random _distributed node_, and blocks the calling process.
* `{name}_local_async`, when called, spawns the function in a _node local_ process, handing back a wrapped reference to the process, which can be polled, or blocked upon.
* `{name}_remote_async`, when called, spawns the function in a process on a random _distributed node_, handing back a wrapped reference to the process, which can be polled, or blocked upon.
* `{name}_remote_async_fanout`, which takes a `Vec` of arg tuples and round-robin distributes calls to that function with those arguments, polling all of the processes, and blocking until all of completed with a `Vec` of the results.

Basically, I want to make it eay to just annotate methods, and then call methods that "just work" in a distributed way.

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