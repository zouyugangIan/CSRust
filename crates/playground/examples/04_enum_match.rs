fn main() {
    let language = Language::Chinese;
    match language {
        Language::English => println!("Hello!"),
        Language::Russian => println!("Привет!"),
        Language::Chinese => println!("你好!"),
        Language::Spanish => println!("Hola!"),
        Language::French => println!("Bonjour!"),
        Language::German => println!("Hallo!"),
        Language::Japanese => println!("こんにちは!"),
        Language::Portuguese => println!("Olá!"),
        _ => println!("Unknown language!"),
    }
    println!("language is {}", language.getLanguage());
}

impl Language {
    fn getLanguage(&self) -> &str {
        match self {
            Language::English => "English",
            Language::Russian => "Russian",
            Language::Chinese => "Chinese",
            Language::Spanish => "Spanish",
            Language::French => "French",
            Language::German => "German",
            Language::Japanese => "Japanese",
            Language::Portuguese => "Portuguese",
        }
    }
}
enum Language {
    English,
    Russian,
    Chinese,
    Spanish,
    French,
    German,
    Japanese,
    Portuguese,
}
