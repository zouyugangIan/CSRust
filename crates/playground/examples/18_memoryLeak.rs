use std::cell::RefCell;

fn main() {
    let x = 5;
    let y = RefCell::new(&x);
    println!("x:{:?}", x);
    println!("y:{:?}", y.borrow());
}
