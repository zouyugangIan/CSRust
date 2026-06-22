use std::io;

fn main() {
    loop {
        let mut input = String::new();
        println!("Enter a temperature in Fahrenheit:");
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");
        covert_fto_s();

        let x = 5;
        let y = {
            let x = 3;
            x + 1
        };
        println!("The value of y is: {y}");
        println!("The value of x is: {x}");
    }
}

fn covert_fto_s() {
    let mut input = String::new();
    println!("Enter a temperature in Fahrenheit:");
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    let f: f64 = input.trim().parse().expect("Please type a number!");
    let s = (f - 32.0) * 5.0 / 9.0;
    println!("Temperature in Celsius: {s}");
}
fn covert_sto_f() {
    let mut input = String::new();
    println!("Enter a temperature in Celsius:");
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    let s: f64 = input.trim().parse().expect("Please type a number!");
    let f = s * 9.0 / 5.0 + 32.0;
    println!("Temperature in Fahrenheit: {f}");
}
