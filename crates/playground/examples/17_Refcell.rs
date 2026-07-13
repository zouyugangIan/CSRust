use std::cell::RefCell;

fn main() {
    let x = 5;
    let y = RefCell::new(&x);
    println!("x:{:?}", x);
    println!("y:{:?}", y.borrow());
}

pub trait Messenger {
    fn send(&self, msg: &str);
}

pub struct LimitTracker<'a, T: Messenger> {
    messenger: &'a T,
    value: usize,
    max: usize,
}

impl<'a, T: Messenger> LimitTracker<'a, T> {
    pub fn new(messenger: &'a T, max: usize) -> Self {
        Self {
            messenger,
            value: 0,
            max,
        }
    }

    pub fn set_value(&mut self, value: usize) {
        self.value = value;

        let percentage_of_max = self.value as f64 / self.max as f64;

        if percentage_of_max >= 1.0 {
            println!("Value exceeds max: {}", percentage_of_max);
        } else if percentage_of_max >= 0.9 {
            self.messenger.send("Warning: value is nearing max");
        } else if percentage_of_max >= 0.75 {
            self.messenger.send("Warning: value is nearing 75% of max");
        } else {
            self.messenger.send("Value is below max");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMessenger {
        sent_messages: Vec<String>,
    }
    impl MockMessenger {
        fn new() -> MockMessenger {
            MockMessenger {
                sent_messages: vec![],
            }
        }
    }

    impl Messenger for MockMessenger {
        fn send(&self, msg: &mut str) {
            self.sent_messages.push(msg.to_string());
        }
    }

    #[test]
    fn it_sends_an_over_75_percent_warning_message() {
        let mock_messenger = MockMessenger::new();
        let mut limit_tracker = LimitTracker::new(&mock_messenger, 100);

        limit_tracker.set_value(80);

        assert_eq!(mock_messenger.sent_messages.len(), 1);
    }
}
