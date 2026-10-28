//! A job whose node goes down panics its caller instead of hanging it.
//!
//! Run it inside a `lunatic node` with one peer (see the `pythagorean` example), and kill the peer once it prints
//! "lingering": the app should panic with "job `linger` on node ... was lost".

use std::time::Duration;

fn main() {
    let result = linger_remote();
    println!("unreachable: {result}");
}

#[lucidity::job]
fn linger() -> u64 {
    println!("lingering on node {}", lucidity::local_node());
    lucidity::lunatic::sleep(Duration::from_secs(600));
    0
}
