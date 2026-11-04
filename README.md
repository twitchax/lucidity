[![Build and Test](https://github.com/twitchax/lucidity/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/lucidity/actions/workflows/build.yml)
[![Version](https://img.shields.io/crates/v/lucidity.svg)](https://crates.io/crates/lucidity)
[![Crates.io](https://img.shields.io/crates/d/lucidity?label=crate)](https://crates.io/crates/lucidity)
[![Documentation](https://docs.rs/lucidity/badge.svg)](https://docs.rs/lucidity)
[![Rust](https://img.shields.io/badge/rust-nightly-blue.svg?maxAge=3600)](https://github.com/twitchax/lucidity/blob/main/rust-toolchain.toml)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# lucidity

A distributed execution engine built upon [lunatic](https://github.com/lunatic-solutions/lunatic).

## Motivation

`lunatic` by itself is a set of low-level features: a runtime, syscalls, and language wrappers.

However, the `Process` architecture is a bit harder to use when trying to keep code readable.  This library provides a `proc-macro`, and helpers for fly.io (the `fly` feature), to make it easier to write distributed code
on top of the excellent lunatic runtime.

### Example

Here is a simple example.

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
    ((square_remote_async(a).await_get() + square_remote_async(b).await_get()) as f32).sqrt()
}

#[lucidity::job]
fn square(a: u32) -> u32 {
    a * a
}
```

For each method you place the proc macro (`lucidity::job`) on, we generate a few others.

* `{name}_local`, when called, spawns the function in a _node local_ `Process`, and blocks the calling `Process`.
* `{name}_remote`, when called, spawns the function in a `Process` on a random other _distributed node_, and blocks the calling `Process`.
* `{name}_local_async`, when called, spawns the function in a _node local_ `Process`, handing back a `lucidity::Job`, which can be checked (`try_get`), or blocked upon (`await_get`).
* `{name}_remote_async`, when called, spawns the function in a `Process` on a random other _distributed node_, handing back a `lucidity::Job`, which can be checked (`try_get`), or blocked upon (`await_get`).
* `{name}_remote_fanout`, which takes a `Vec` of arg tuples, spawns one job per tuple across the other nodes (round-robin by default, see the `fanout` option), and blocks until all are complete, returning a `Vec` of the results in the same order.

Those generated functions can be called like any other function; setting up the `Process`es and `Mailbox`es is handled
for you.  The tradeoff is that this library is opinionated about how you write your code and what you can do with it
(open to suggestions, though).

Under the hood, each job is spawned as a small watcher `Process` on the target node, which spawns the job linked to itself, and
sends exactly one message back: the result, or that the job's `Process` died.  That message goes through a relay `Process` on the
caller's node, which monitors the watcher, so if the target _node_ goes down, the relay reports the job lost instead.  Nothing polls
or retries a job once it started, so a slow job is only slow, and a job that dies (panics, traps, runs out of memory or fuel), or whose node goes down,
panics the caller instead of hanging it.  Dropping a `Job` before it finishes cancels it.

The `remote` functions never pick the calling node; `lucidity::remote_nodes()` lists the nodes they can pick.  When the node they
picked has left the cluster, they (and the `fanout` functions) spawn the job on another one; `lucidity::Job::spawn_any` does the same
for any list of nodes.

## Library Usage

First, install lunatic.  lucidity needs the [twitchax/lunatic](https://github.com/twitchax/lunatic) fork of the
runtime, 0.15 or later: download a binary from its [releases](https://github.com/twitchax/lunatic/releases), or
build it.

```bash
$ cargo install --locked --git https://github.com/twitchax/lunatic --tag v0.15.0 lunatic-runtime
```

Add this to your `Cargo.toml`:

```toml
[dependencies]
lucidity = "0.2"
```

lucidity builds on stable Rust; add the WASI target with `rustup target add wasm32-wasip1`.  In your `.cargo/config.toml`:

```toml
[build]
target = "wasm32-wasip1"

[target.wasm32-wasip1]
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

```bash
#!/bin/bash

/lunatic control --bind-socket [::]:3030 &

/lunatic node --wasm /app.wasm --bind-socket $NODE_REACHABLE_IP:3031 http://[::1]:3030/
```

Then, within your built wasm, you would call `lucidity::fly::ensure_machines(key, count)` (with a fly.io API token) to start
`count` worker machines that join that control node, before running any of the distributed methods.

## `lunatic` Primer

This library is built on top of `lunatic`, so it is important to understand the basics of `lunatic` before using this library.

### Processes

`lunatic` is built around `Process`es: lightweight threads of execution spawned by the runtime.  Each `Process` has its
own isolated memory, and `Process`es communicate by sending messages to each other's `Mailbox`es.

You can still use `Process`es directly alongside `lucidity` when you need them.

### Mailboxes

Each `Process` has a `Mailbox`, a message queue that other `Process`es can send messages to.

You don't need to manage `Mailbox`es directly, as `lucidity` handles them for you.  Still, it helps to know they are
there: `Process`es and `Mailbox`es behave like coroutines or goroutines in other languages, not like async Rust.

As such, the generated methods feel like async or blocking Rust, but blocking here parks the calling `Process` on its
`Mailbox`, not a thread.

### WASM

`lunatic` runs code as WebAssembly (WASM) in a sandboxed environment.  Every node runs the `lunatic` runtime, which
gives your WASM syscalls for communication, and because WASM abstracts away the machine code, any node can run the WASM
from another node.

Multiple nodes could each be started with their own WASM: each node registers its module with the control node, and the
`lunatic` runtime can spawn `Process`es from any of those modules on any node.

### Remote Processes

`Process`es spawned remotely take advantage of the fact that your executable _is_ WASM.  The application node registers
your WASM executable with the control node, and a remote node downloads it from there (once) before spawning a
`Process`, using function pointers to call into your WASM executable.  This is why you need to build your code as WASM,
and why only the application node needs the `.wasm` file.

The `lunatic` runtime distributes your code to _other_ nodes automatically, so your "bare" functions "just work": if a
remote process calls a normal function, that function is already in the WASM on the node where the process is running.

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
    // Calling `square` here does not spawn a process, and is called by the currently executing process
    // as if it were a normal function.
    let result = square(3);

    println!("result: {:#?}", result);
}
```

### Local / Remote Process

If you want to spawn a process locally, you can use the `{name}_local` method.

```rust
fn main() {
    // Fans `pythagorean1` out across the remote nodes (see "Remote Fanout" below).
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
    // Calling `square_local` spawns a local process and blocks the calling process until it completes.
    // This lets the `lunatic` executor yield more often (especially if `square` has yield points),
    // and blocking is cheap in a lightweight process, though `a` and `b` are not squared in parallel.
    ((square_local(a) + square_local(b)) as f32).sqrt()
}

#[lucidity::job]
fn pythagorean2(a: u32, b: u32) -> f32 {
    // Calling `square_remote` spawns a process on a random remote node and blocks the calling process until it
    // completes, so the work is distributed, but `a` and `b` are still squared one after the other.
    ((square_remote(a) + square_remote(b)) as f32).sqrt()
}
```

### Local / Remote Async Process

If you want more fine-grained control over when to block, you can use the `{name}_local_async` and `{name}_remote_async` methods.

```rust
fn main() {
    // Fans `pythagorean1` out across the remote nodes (see "Remote Fanout" below).
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
    // Each call spawns a local process, and hands back a `Job` that can be polled, or blocked upon.
    // Here, we are going to block with `await_get`.  This can be used for either local or remote
    // async jobs.
    let square_a_job = square_local_async(a);
    let square_b_job = square_local_async(b);

    // We get to this point "immediately".  The `square_a_job` and `square_b_job` are running in
    // their own processes, and we can do other work here.

    // Maybe do some other work in here ...

    let square_a = square_a_job.await_get();
    let square_b = square_b_job.await_get();

    ((square_a + square_b) as f32).sqrt()
}

#[lucidity::job]
fn pythagorean2(a: u32, b: u32) -> f32 {
    // Each call spawns a remote process, and hands back a `Job` that can be polled, or blocked upon.
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
        };

        let Some(b) = square_b_job.try_get() else {
            continue;
        };

        break (a, b);
    };

    ((square_a + square_b) as f32).sqrt()
}
```

### Remote Fanout

To run the same operation across a list of arguments and block until all of them complete, use the
`{name}_remote_fanout` method.

```rust
fn main() {
    // Calls `square` across the remote nodes (round-robin by default), running each input in its
    // own process and blocking the calling process until all of them finish.  For example, you could
    // put a batch of images in a `Vec` and call this method to fan out the work.
    let results = square_remote_fanout(vec![1, 2, 3, 4, 5]);

    println!("result: {:#?}", results);
}
```

## Job Attribute Options

The `lucidity::job` proc macro has a few options that can be used to customize the behavior of the generated methods.

Generally, these do not need to be used, but they are available if you need them.

* `memory`: The maximum memory a job's `Process` may use.  Defaults to `100 * 1024 * 1024` (100 MiB).
* `fuel`: The maximum fuel a job's `Process` may use in total (one unit of fuel is about 100,000 WASM instructions).  Defaults to no limit.  A job that runs out of fuel dies, which panics its caller.
* `fanout`: How `{name}_remote_fanout` picks nodes.  Defaults to `"roundrobin"`.  The other option is `"random"`.

The `*_retry_interval_ms` options from `0.1` were removed, since nothing retries anymore; using one is a compile error.

## Feature Flags

* `fly`: This enables the `lucidity::fly` module, which allows you to use the `fly.io` platform to automatically set up nodes
  from the main `lunatic` node: `ensure_machines(key, count)` starts `count` worker machines (named `lucid-1` and up) from
  the `twitchax/lunatic` image (`docker/Dockerfile`) and joins them to the control node on the calling machine's port 3030,
  and `list_machines(key, app_name)` lists an app's machines.  This is not enabled by default, as it requires a `fly.io`
  account, and a bit of setup.

## Test

See [DEVELOPMENT.md](DEVELOPMENT.md): the end-to-end tests run the examples on a real local cluster.

## Thanks

Special thanks to [lunatic](https://github.com/lunatic-solutions/lunatic)'s authors and contributors for their excellent work,
and special thanks to the primary author, [bkolobara](https://github.com/bkolobara).

## License

MIT