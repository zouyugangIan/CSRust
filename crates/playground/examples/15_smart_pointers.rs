use crate::List::{Cons, Nil};

fn main() {
    let b = Box::new(5);
    println!("the value of box is:{b}");
    let consList = Cons(1, Box::new(Cons(2, Box::new(Cons(3, Box::new(Nil))))));
    println!("{:?}", consList)
}

#[derive(Debug)]
enum List {
    Cons(i32, Box<List>),
    Nil,
}

#[test]
fn test_main() {
    let x = 5;
    let y = Box::new(x);

    assert_eq!(5, x);
    assert_eq!(5, *y);
}
