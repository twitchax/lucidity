//! A job whose process dies (here, by running out of fuel) panics its caller instead of hanging it.
//!
//! `cargo run --target wasm32-wasip1 --example job_dies` should panic with "job `spin` on node ... died".

fn main() {
    println!("spinning");
    let result = spin_local();
    println!("unreachable: {result}");
}

#[lucidity::job(fuel = 1)]
fn spin() -> u64 {
    (0..u64::MAX).fold(0, |acc, k| std::hint::black_box(acc ^ k))
}
