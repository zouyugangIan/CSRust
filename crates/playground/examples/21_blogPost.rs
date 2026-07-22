fn main() {}

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
    fn publish(&mut self) -> &mut dyn State;
}

struct Draft {}

impl State for Draft {
    fn publish(&mut self) -> &mut dyn State {}
}

struct PendingReview {}

impl State for PendingReview {
    fn publish(&mut self, post: &mut dyn Draw) {}
}

struct Approved {}

impl State for Approved {
    fn publish(&mut self, post: &mut dyn Draw) {}
}
