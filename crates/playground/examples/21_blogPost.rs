fn main() {}

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

pub struct Post {
    state: Box<dyn State>,
    content: String,
}
impl Post {
    pub fn new(content: String) -> Self {
        Self {
            state: Box::new(Draft {}),
            content,
        }
    }
    pub fn publish(&mut self) {
        self.state.publish(self);
    }
}

trait State {
    fn publish(&mut self, post: &mut dyn Draw);
}

struct Draft {}

impl State for Draft {
    fn publish(&mut self, post: &mut dyn Draw) {}
}
