use iced::{executor, image, Application, Clipboard, Command, Element, Row, Settings, Text};

struct RimView {
    image: image::Handle,
    image_viewer: image::viewer::State,
}

impl Application for RimView {
    type Executor = executor::Default;
    type Message = ();
    type Flags = ();
    fn new(_flags: ()) -> (RimView, Command<Self::Message>) {
        (
            RimView {
                image: image::Handle::from_path(
                    "C:/Users/shafeib/Desktop/2_1_1_2022-01-10_14-31-13.png",
                ),
                image_viewer: image::viewer::State::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("A cool application")
    }

    fn update(
        &mut self,
        _message: Self::Message,
        _clipboard: &mut Clipboard,
    ) -> Command<Self::Message> {
        Command::none()
    }

    fn view(&mut self) -> Element<Self::Message> {
        Row::new()
            .push(image::Viewer::new(
                &mut self.image_viewer,
                self.image.clone(),
            ))
            .into()
    }
}

fn main() -> iced::Result {
    RimView::run(Settings::default())
}
