use rayon::prelude::*;
use std::hint::black_box;
use std::time::Instant;

#[inline(never)]
fn work(mut x: u64) -> u64 {
    x = black_box(x);

    for _ in 0..100 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    }

    black_box(x)
}

fn main() {
    let total = 10_000_000u64;

    println!(
        "build mode: {}",
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "RELEASE"
        }
    );

    println!("rayon threads: {}", rayon::current_num_threads());
    println!("items: {total}");
    println!();

    // -------------------------
    // 单线程
    // -------------------------
    let start = Instant::now();

    let single: u64 = (0..total).map(work).fold(0u64, u64::wrapping_add);

    let single_time = start.elapsed();

    println!("single result  : {}", black_box(single));
    println!("single elapsed : {:.6} s", single_time.as_secs_f64());

    // -------------------------
    // Rayon
    // -------------------------
    let start = Instant::now();

    let parallel: u64 = (0..total)
        .into_par_iter()
        .map(work)
        .reduce(|| 0u64, u64::wrapping_add);

    let parallel_time = start.elapsed();

    println!("parallel result: {}", black_box(parallel));
    println!("parallel elapsed: {:.6} s", parallel_time.as_secs_f64());

    println!();

    println!(
        "speedup: {:.2}x",
        single_time.as_secs_f64() / parallel_time.as_secs_f64()
    );
}
