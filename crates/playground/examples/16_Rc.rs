use std::rc::Rc;

use crate::List::{Cons, Nil};

fn main() {
    let example = Rc::new(5);
    println!("the example value of Rc is:{example}");
    let a = Rc::new(Cons(1, Rc::new(Cons(2, Rc::new(Cons(3, Rc::new(Nil)))))));
    println!("count after creation of a: {}", Rc::strong_count(&a));
    let b = Cons(4, Rc::clone(&a));
    println!("count after creation of b: {}", Rc::strong_count(&a));
    {
        let c = Cons(4, Rc::clone(&a));
        println!("count after creation of c: {}", Rc::strong_count(&a));
        println!("c:{:?}", c);
    }
    println!("count after c goes out of scope:{}", Rc::strong_count(&a));
    println!("a:{:?}", a);
    println!("b:{:?}", b);
}

#[derive(Debug)]
enum List {
    Cons(i32, Rc<List>),
    Nil,
}

#[test]
fn test_main() {
    let x = 5;
    let y = Rc::new(x);

    assert_eq!(5, x);
    assert_eq!(5, *y);
}
