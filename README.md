[![Build and Test](https://github.com/twitchax/lucidity/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/lucidity/actions/workflows/build.yml)
[![Version](https://img.shields.io/crates/v/lucidity.svg)](https://crates.io/crates/lucidity)
[![Crates.io](https://img.shields.io/crates/d/lucidity?label=crate)](https://crates.io/crates/lucidity)
[![Documentation](https://docs.rs/lucidity/badge.svg)](https://docs.rs/lucidity)
[![Rust](https://img.shields.io/badge/rust-nightly-blue.svg?maxAge=3600)](https://github.com/twitchax/rtz)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# lucidity

A distributed execution engine built upon [lunatic](https://github.com/lunatic-solutions/lunatic).

## Motivation

Basically, `lunatic` by itself is a set of "low-level" features: runtime, syscalls, and language-wrappers.

However, the `Process` architecture is a bit harder to use when trying to keep code readable.  This library provides a `proc-macro`, and, eventually, some helpers for common platforms like fly.io, to make it easier to write distributed code
on top of the excellent lunatic runtime.

### Example

Here is a simple example below.

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

For each method you place the proc macro (`lucidity::job`) on, we generate a few others.

* `{name}_local`, when called, spawns the function in a _node local_ `Process`, and blocks the calling `Process`.
* `{name}_remote`, when called, spawns the function in a `Process` on a random _distributed node_, and blocks the calling `Process`.
* `{name}_local_async`, when called, spawns the function in a _node local_ `Process`, handing back a wrapped reference to the `Process`, which can be polled, or blocked upon.
* `{name}_remote_async`, when called, spawns the function in a `Process` on a random _distributed node_, handing back a wrapped reference to the `Process`, which can be polled, or blocked upon.
* `{name}_remote_fanout`, which takes a `Vec` of arg tuples and roundrobin distributes calls to that function with those arguments, polling all of the `Process`es, and blocking until all are complete, returning a `Vec` of the results.

The above example uses the `lucidity::job` proc macro to generate a few of those functions, and they can be "called" like any other function.  The goal here is to use the excellent architecture of `lunatic`, while cutting down on some of the
boilerplate required to successfully write the distributed code.  Setting up the `Process`es, and the `Mailbox`es, etc., is all handled for you.  
The tradeoff is that this library is opinionated about how you write your code, and what you can do with it (open to suggestions, though).  In addition, this library introduces some simple loops with timeouts to avoid possible deadlock,
which has some overhead.

## Library Usage

First, install lunatic.

```bash
$ cargo install lunatic-runtime
```

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

### Distributed Setup

To use this library in a distributed setup, you will need to do a few things.  This example could easily be used locally
as well, by just using the loopback address.

First, you need to run the control node somewhere.

```bash
$ lunatic control --bind-socket [::]:3030
```

And, on any other machines where you want the remote methods to run, you will need to set up nodes.

```bash
$ lunatic node --bind-socket [::]:3031 http://{IP_OR_HOST_OF_CONTROL}:3030/
```

#### Local Testing

For testing, you would then build your code and run it inside a `lunatic` node.  Something like this.

```bash
cargo build --release && lunatic node --wasm path/to/built/wasm/exe.wasm --bind-socket [::]:3032 http://{IP_OR_HOST_OF_CONTROL}:3030/
```

#### Production Setup

In a more production setup, you would probably use something like `fly.io` to deploy your code (use the `fly` feature), and you may want
to build and run your code in a container.  The easiest way is a simple docker container that runs the control node, and the application
node.  Your entry point would look something like this.

> NOTE: Due to UDP issues on fly.io, the "automatic" fly.io setup feature does not work, but will be enabled when I get the issues
> resolved.

```bash
#!bin/bash

/lunatic control --bind-socket [::]:3030 &

/lunatic node --wasm /irl_processor.wasm --bind-socket $NODE_REACHABLE_IP:3031 http://[::1]:3030/
```

Then, within your built wasm, you would span some nodes that would connect to the control node on other machines
before running any of the distributed methods.

## `lunatic` Primer

This library is built on top of `lunatic`, so it is important to understand the basics of `lunatic` before using this library.

### Processes

`lunatic` is built around the concept of `Processes`.  A `Process` is a lightweight thread of execution that is spawned
by the runtime.  Each `Process` has its own stack, and is isolated from other `Process`es.  `Process`es communicate
with each other via `Mailbox`es, which are essentially queues that can be used to send messages between `Process`es.

In the case of this library, you can totally use `Process`es directly, but the point of the `lucidity` library is to
make it easier to write distributed code, so we will focus on that.

### Mailboxes

`Mailbox`es are the primary way that `Process`es communicate with each other.  A `Mailbox` is a queue that can be used
to send messages between `Process`es.  Each `Process` has a `Mailbox` that can be used to send messages to that `Process`.

For the purposes of this library, you don't need to worry about `Mailbox`es, as they are handled for you.  However, it is
important to know that they exist since the "syntactic sugar" provided by this library abstracts away these mssage queues.
This is not like "async Rust", or any other "async/await" type languages.  These `Process`es, and their `Mailbox`es, are
more like the coroutine or goroutine behavior of other languages.

As such, this library adds some overhead in the way it "feels" sort of like async Rust or blocking Rust, but it achieves
that feel by using timeouts with wait loops.  As this project is meant more for "fanning out" rigorous work to other nodes, this 
overhead is acceptable, but it is important to understand that this is not like "async Rust".

### WASM

`lunatic` is built around the concept of WebAssembly (WASM).  WASM is a binary format that is meant to be run in a sandboxed
environment.  `lunatic` is able to scale so well to a distributed model because it relies on the concept that the "runtime"
ships with a WASM runtime, while that WASM code can make certain "runtime syscalls" for communication.  The WASM abstracts
away the machine code such that each node can function properly with just the WASM from another node.

Theoretically, multiple nodes could each be initialized with their own WASM, and the `lunatic` runtime would be able to
spawn `Process`es on any of those nodes, as each node would send its WASM to the other nodes.

### Remote Processes

`Process`es that are spawed remotely take advantage of the fact that your executable _is_ WASM.  Basically, `lunatic`
sends a copy of your WASM executable to the remote node, and then spawns a `Process` there, essentially using function
pointers to call the functions in your WASM executable.  This is why you need to build your code as WASM, and why you
need to run the control node, and the application node, with the same executable.

However, you don't need to worry about getting your code onto _other_ nodes.  The `lunatic` runtime handles this automatically.
This also means that your "bare" functions "just work".  That function is in the WASM, so if a process calls that function,
it will be called on the node where the process is running since that node _has_ the WASM.

Pretty cool, right?

## Examples

Let's look at a few examples to understand when you would use specific types of methods.

For all of these examples, we can assume that we have declared the `square` function like this.

```rust
#[lucidity::job]
fn square(a: u32) -> u32 {
    a * a
}
```

### "No Process"

Even if you mark a function with the `lucidity::job` proc macro, you can still call it like a normal function.

```rust
fn main() {
    // Calling `square` here does not span a process, and is called by the currently executing process
    // as if it were a normal function.
    let result = square(3);

    println!("result: {:#?}", result);
}
```

### Local / Remote Process

If you want to spawn a process locally, you can use the `{name}_local` method.

```rust
fn main() {
    // The `remote_fanout` is discussed below, but this is sort of the main meat and potatoes.
    // This function will be called on a set of nodes (round-robin-ed), and each of those nodes
    // will run it in a process.
    let results = pythagorean1_remote_fanout(vec![
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
fn pythagorean1(a: u32, b: u32) -> f32 {
    // Calling `square_local` here spawns a process locally, and blocks the current process until the
    // spawned process completes.  This is great for allowing the `lunatic` executor to "yield" more often,
    // especially if the `square` method had reasonable yield points.  However, the process it is called from is blocked,
    // so keep that in mind.
    //
    // In this case, we don't really mind blocking here, since we are in a lightweight process.  However, you may notice
    // there is an inefficiency in not computing the square of `a` and `b` in parallel.
    ((square_local(a) + square_local(b)) as f32).sqrt()
}

#[lucidity::job]
fn pythagorean2(a: u32, b: u32) -> f32 {
    // Calling `square_remote` here spawns a process on a random remote node, and blocks the current process until the
    // spawned process completes.  This is great for ensuring a certain set of processes are being distributed,
    // but blocks the process it is called from.
    ((square_remote(a) + square_remote(b)) as f32).sqrt()
}
```

### Local / Remote Async Process

If you want more fine-grained control over when to block, you can use the `{name}_local_async` and `{name}_remote_async` methods.

```rust
fn main() {
    // The `remote_fanout` is discussed below, but this is sort of the main meat and potatoes.
    // This function will be called on a set of nodes (round-robin-ed), and each of those nodes
    // will run it in a process.
    let results = pythagorean1_remote_fanout(vec![
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
fn pythagorean1(a: u32, b: u32) -> f32 {
    // This spawns a local process, and hands back a `Job` that can be polled, or blocked upon.
    // Here, we are going to block with `await_get`.  This can be used for either local or remote
    // async jobs.
    let square_a_job = square_local_async(a);
    let square_b_job = square_local_async(b);

    // We get to this point "immediately".  The `square_a_job` and `square_b_job` are running in
    // their own processes, and we can do other work here.

    // Maybe do some other work in here ...

    let mut square_a = square_a_job.await_get();
    let mut square_b = square_b_job.await_get();

    ((square_a + square_b) as f32).sqrt()
}

#[lucidity::job]
fn pythagorean2(a: u32, b: u32) -> f32 {
    // This spawns a local process, and hands back a `Job` that can be polled, or blocked upon.
    // Here, we are going to loop and check for completion with `try_get` (sort of naively).  This can be used for either local or remote
    // async jobs.
    let square_a_job = square_remote_async(a);
    let square_b_job = square_remote_async(b);

    // We get to this point "immediately".  The `square_a_job` and `square_b_job` are running in
    // their own processes, and we can do other work here.
    
    // Maybe do your own looping ...
    let (square_a, square_b) = loop {
        let Some(a) = square_a_job.try_get() else {
            continue;
        }

        let Some(b) = square_b_job.try_get() else {
            continue;
        }

        break (a, b);
    }

    ((square_a + square_b) as f32).sqrt()
}
```

### Remote Fanout

If you essentially want to do the same operation, but with different arguments, and you want to block on all of them,
you can use the `{name}_remote_fanout` method.

```rust
fn main() {
    // The `remote_fanout` is discussed below, but this is sort of the main meat and potatoes.
    // This function will be called on a set of nodes (round-robin-ed), and each of those nodes
    // will run it in a process.
    //
    // This is great for ensuring a certain set of processes are being distributed, but blocks the process it is called from.
    // You may have something where you are processing a set of images.  This would be a great use case to put those images
    // in a `Vec`, and then call this method to fan out all of the work.
    let results = square_remote_fanout(vec![1, 2, 3, 4, 5]);

    println!("result: {:#?}", results);
}
```

## Job Attribute Options

The `lucidity::job` proc macro has a few options that can be used to customize the behavior of the generated methods.

Generally, this do not need to be used, but they are available if you need them.

* `init_retry_interval_ms`: This is the number of milliseconds to wait between retries when trying to initialize a `Process`.  Defaults to `100`.
* `sync_retry_interval_ms`: This is the number of milliseconds to wait between retries when trying to get a blocking (e.g., `{name}_local` or `{name}_remote`) from a `Process`.  Defaults to `100`.
* `async_init_retry_interval_ms`: This is the number of milliseconds to wait between retries when trying to initialize a `Process` asynchronously (e.g., `{name}_local_async` or `{name}_remote_async`).  Defaults to `100`.
* `async_get_retry_interval_ms`: This is the number of milliseconds to wait between retries when trying to get a non-blocking result (e.g., `{name}_local_async` or `{name}_remote_async`) from a `Process`.  Defaults to `100`.
* `async_set_retry_interval_ms`: This is the number of milliseconds to wait between retries when the execution `Process` attempts to set a non-blocking result (e.g., `{name}_local_async` or `{name}_remote_async`) from a `Process`.  Defaults to `100`.
* `shutdown_retry_interval_ms`: This is the number of milliseconds to wait between retries when trying to shutdown a `Process`.  Defaults to `100`.
* `memory`: This is the amount of maximum memory allowed to the `Process`.  Defaults to `100 * 1024 * 1024` (100MB).
* `fuel`: This is the amount of maximum fuel allowed to the `Process`.  Defaults to `10` (each unit of fuel is approximately 100,000 WASM instructions).
* `fanout`: This is the type of scheme to use when fanning out.  Defaults to `roundrobin`.  The other option is `random`.

## Feature Flags

* `fly`: This enables the `fly` feature, which allows you to use the `fly.io` platform to automatically set up nodes
  from the main `lunatic` node.  This is not enabled by default, as it requires a `fly.io` account, and a bit of setup.
  See the `fly.io` documentation for more information.
  > NOTE: This functionality is also (currently) rendered useless by limitations with UDP on `fly.io`.  I am working with
  > the `fly.io` team in the forums to resolve this issue.

## Test

```bash
cargo test
```

## Thanks

Special thanks to the [lunatic](https://github.com/lunatic-solutions/lunatic)'s authors and contributors for their excellent work,
and special thanks to the primary author, [bkolobara](https://github.com/bkolobara).

## License

MIT