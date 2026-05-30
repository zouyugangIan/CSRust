use std::io;
fn main() {
    loop {
        let mut input = String::new();
        println!("Enter a number to calculate the Fibonacci number:");
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");
        let n: u32 = match input.trim().parse() {
            Ok(num) => num,
            Err(_) => {
                println!("Please type a number!");
                return;
            }
        };
        let fibonacci_number_rec = fibonacci_rec(n);
        let fibonacci_number_iter = fibonacci_iter(n);
        println!("Fibonacci number recursive: {fibonacci_number_rec}");
        println!("Fibonacci number iterative: {fibonacci_number_iter}");
    }
}

fn fibonacci_rec(n: u32) -> u32 {
    if n <= 1 {
        return n;
    } else {
        return fibonacci_rec(n - 1) + fibonacci_rec(n - 2);
    }
}

fn fibonacci_iter(n: u32) -> u32 {
    if n <= 1 {
        return n;
    }
    let mut a = 0;
    let mut b = 1;
    for _ in 0..n - 1 {
        let c = a + b;
        a = b;
        b = c;
    }
    return b;
}
