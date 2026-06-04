fn main() {
    let mut vec = vec![1, 2, 3, 4, 5];

    for i in 0..vec.len() {
        println!("{ }", &vec[i]);
        vec[i] += 1;
        vec.push(vec[i + 1]);
        println!("{}", vec.last().unwrap());
    }
    for element in vec.iter() {
        println!("the value is:{}", element);
        println!("{}", element);
    }
}
