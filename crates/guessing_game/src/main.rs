use rand::Rng;
use std::cmp::Ordering;
use std::io;

fn main() {
    println!("猜数字游戏");
    println!("我已经想好了一个 1 到 100 的数字。\n");

    let secret_number = rand::thread_rng().gen_range(1..=100);

    loop {
        println!("请输入你的猜测:");

        let mut guess = String::new();
        io::stdin().read_line(&mut guess).expect("读取输入失败");

        let guess: u32 = match guess.trim().parse() {
            Ok(number) => number,
            Err(_) => {
                println!("请输入有效的正整数。\n");
                continue;
            }
        };

        println!("你猜的是: {guess}");

        match guess.cmp(&secret_number) {
            Ordering::Less => println!("太小了!\n"),
            Ordering::Greater => println!("太大了!\n"),
            Ordering::Equal => {
                println!("恭喜你，猜对了!");
                break;
            }
        }
    }
}
