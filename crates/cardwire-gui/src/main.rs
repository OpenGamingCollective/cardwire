use iced::{self, widget};

struct Counter {
    count: i32,
}
impl Counter {
    fn new() -> Self {
        // initialize the counter struct
        // with count value as 0.
        Self { count: 0 }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        // handle emitted messages
        match message {
            Message::IncrementCount => self.count += 1,
            Message::DecrementCount => self.count -= 1,
        }

        iced::Task::none()
    }
    fn view(&self) -> iced::Element<'_, Message> {
        // create the View Logic (UI)
        let row = widget::row![
            widget::button("-").on_press(Message::DecrementCount),
            widget::text!("Count: {}", self.count),
            widget::button("+").on_press(Message::IncrementCount)
        ]
        .spacing(10);

        widget::container(row).center(iced::Length::Fill).into()
    }
}

#[derive(Debug, Clone, Copy)]
enum Message {
    // Emitted when the increment ("+") button is pressed
    IncrementCount,
    // Emitted when decrement ("-") button is pressed
    DecrementCount,
}

fn main() -> iced::Result {
    iced::application(Counter::new, Counter::update, Counter::view)
        .title("Example")
        .run()
}
