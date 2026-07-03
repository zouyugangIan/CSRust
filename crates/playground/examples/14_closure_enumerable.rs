use std::thread;
use std::time::Duration;

//Rust的闭包（closures）是可以保存进变量或作为参数传递给其他函数的匿名函数。允许捕获调用者作用域中的值。
//我们希望能够在程序的一个位置指定某些代码，并只在程序的某处实际需要结果的时候执行这些代码。这正是闭包的用武之地！

fn main() {
    let simulated_user_specified_value = 10;
    let simulated_random_number = 7;

    generate_workout(simulated_user_specified_value, simulated_random_number);
}

fn simulated_expensive_calculation(intensity: u32) -> u32 {
    println!("calculating slowly...");
    thread::sleep(Duration::from_secs(2));
    intensity
}

fn generate_workout(intensity: u32, random_number: u32) {
    let expensive_closure = |num| {
        println!("calculating slowly...");
        thread::sleep(Duration::from_secs(2));
        num + 166
    };
    if intensity < 25 {
        println!("Today, do {} pushups!", expensive_closure(intensity));
        println!("Next, do {} situps", expensive_closure(intensity));
    } else {
        if random_number == 3 {
            println!("Take a break today! Remember to stay hydrated!");
        } else {
            print!("Today, run for{}minutes!", expensive_closure(intensity));
        }
    }
}
