use std::error::Error;
use std::fs;

pub struct Config {
    query: String,
    filename: String,
}
 impl Config {
     pub fn new(args: &[String]) -> Config {
        let query = args[1].clone();
        let filename = args[2].clone();

        Config { query, filename }
    }
}

pub fn run(config:Config)-> Result<(),Box<dyn Error>> {
    println!("searching in file: {}", config.filename);
    println!("searching for: {}", config.query);

    let contents = fs::read_to_string(config.filename)?;
    println!("The content of the file is :\n{}", contents);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_result(){
        let query = "duct";
        let contents = "\
    Rust:\
    safe,fast,productive.\
    pick three.";

        assert_eq!(
            vec!["safe,fast,productive."],
            search(query,contents)
        );
    }
}

pub fn search<'a>(query: &str, contents: &'a str)-> Vec<&'a str> {
    vec![]
}