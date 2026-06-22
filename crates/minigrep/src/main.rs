use std::env;
use std::process;
use minigrep::Config;

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{:?}!", args);

    let config = Config::new(&args);
     if let Err(e) =  minigrep:: run(config){
         println!("Application error:{}",e);
         process::exit(1);
     }
}

