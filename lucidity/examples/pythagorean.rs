//! Runs every generated job flavor and checks the results.
//!
//! The local flavors run anywhere; the remote flavors are skipped when no other nodes are connected:
//!
//! ```bash
//! cargo run --target wasm32-wasip1 --example pythagorean
//! ```
//!
//! To run the remote flavors too, run it inside a `lunatic node` registered with a control node that has peers:
//!
//! ```bash
//! cargo build --target wasm32-wasip1 --example pythagorean
//! lunatic control --bind-socket [::1]:3030 &
//! lunatic node --bind-socket [::1]:3031 http://[::1]:3030/ &
//! lunatic node --wasm target/wasm32-wasip1/debug/examples/pythagorean.wasm --bind-socket [::1]:3032 http://[::1]:3030/
//! ```

fn main() {
    assert_eq!(square(3), 9);
    assert_eq!(square_local(3), 9);
    assert_eq!(square_local_async(4).await_get(), 16);
    assert_eq!(pythagorean_local(3, 4), 5.0);
    println!("local: ok");

    if lucidity::lunatic::distributed::nodes().is_empty() {
        println!("remote: skipped (no peer nodes)");
        return;
    }

    assert_eq!(square_remote(5), 25);
    assert_eq!(square_remote_async(6).await_get(), 36);
    assert_eq!(pythagorean_remote_fanout(vec![(3, 4), (6, 8), (5, 12)]), vec![5.0, 10.0, 13.0]);
    assert_eq!(pythagorean_spread_remote_fanout(vec![(8, 15), (7, 24), (20, 21)]), vec![17.0, 25.0, 29.0]);
    println!("remote: ok");
}

/// Squares on the node it runs on.
#[lucidity::job]
fn pythagorean(a: u32, b: u32) -> f32 {
    ((square_local_async(a).await_get() + square_local_async(b).await_get()) as f32).sqrt()
}

/// Squares on random nodes (remote jobs spawning remote jobs).
#[lucidity::job]
fn pythagorean_spread(a: u32, b: u32) -> f32 {
    ((square_remote_async(a).await_get() + square_remote_async(b).await_get()) as f32).sqrt()
}

#[lucidity::job]
fn square(a: u32) -> u32 {
    a * a
}
