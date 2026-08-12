// fn main() {
//     println!("forever");
//     let mut n: u32 = 0;
//     loop {
//         print!("and ever..");
//         n += 1;
//         if n > 1000000000 {
//             println!("breakable-n={}", n);
//             break;
//         }
//     }
// }

use rayon::prelude::*;
use std::time::Instant;

fn main() {
    let total = 1_000_000_000u64;

    let start = Instant::now();
    let single: u64 = (0..total).into_par_iter().map(|x| x).sum();
    println!("single: {single}");
    println!("single elapsed: {:.3} s", start.elapsed().as_secs_f64());

    let start = Instant::now();

    let parallel: u64 = (0..total).into_par_iter().map(|x| x).sum();

    println!("parallel: {parallel}");
    println!("parallel elapsed: {:.3} s", start.elapsed().as_secs_f64());
}
