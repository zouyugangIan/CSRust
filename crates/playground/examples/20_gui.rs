// use gui::Screen;
// fn main() {
//     let screen = Screen {
//         components: vec![Box::new(String::from("Hi"))],
//     };
//     screen.run();
// }

fn main() {
    let screen = Screen {
        components: vec![
            Box::new(Button {
                width: 25,
                height: 10,
                label: String::from("zyg"),
            }),
            Box::new(SelectBox {
                width: 75,
                height: 10,
                option: vec![
                    String::from("slint"),
                    String::from("leptos"),
                    String::from("dioxus"),
                    String::from("egui"),
                ],
            }),
        ],
    };
    screen.run();
}

pub trait Draw {
    fn draw(&self);
}

pub struct Screen {
    pub components: Vec<Box<dyn Draw>>,
}

impl Screen {
    pub fn run(&self) {
        for component in &self.components {
            component.draw();
        }
    }
}

pub struct Button {
    width: u32,
    height: u32,
    label: String,
}

impl Button {
    pub fn new(width: u32, height: u32, label: String) -> Button {
        Button {
            width,
            height,
            label,
        }
    }
}
impl Draw for Button {
    fn draw(&self) {
        println!("Draw Button!");
    }
}

pub struct SelectBox {
    width: u32,
    height: u32,
    option: Vec<String>,
}

impl SelectBox {
    pub fn new(width: u32, height: u32, option: Vec<String>) -> SelectBox {
        SelectBox {
            width,
            height,
            option,
        }
    }
}
impl Draw for SelectBox {
    fn draw(&self) {
        println!("Draw SelectBox!");
    }
}
