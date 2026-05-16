fn main() {
    println!("Rust variable shadowing");
    println!("----------------------");

    show_scope_shadowing();
    println!();
    show_type_change_with_shadowing();
}

fn show_scope_shadowing() {
    println!("Example 1: shadowing in nested scopes");

    let x = 5;
    println!("1) start: x = {x}");

    let x = x + 1;
    println!("2) after shadowing once: x = {x}");

    {
        let x = x * 2;
        println!("3) inside the inner scope: x = {x}");
        println!("this x exists only inside these braces");
    }

    println!("4) back in the outer scope: x = {x}");
}

fn show_type_change_with_shadowing() {
    println!("Example 2: shadowing can also change types");

    let spaces = "   ";
    println!("1) spaces starts as a string: {:?}", spaces);

    let spaces = spaces.len();
    println!("2) after shadowing, spaces becomes a number: {spaces}");
}
