use iced::{
    executor, Application, Command, Element, Length,
    widget::{Column, Container, Row, Scrollable, Text, text_input},
    theme,
};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, time::{sleep, Duration}};
use chrono::{DateTime, Local, Utc};

fn main() -> iced::Result {
    App::run(iced::Settings::default())
}

struct App {
    servers: Vec<Server>,
    history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    timestamp: DateTime<Utc>,
    responses: Vec<Result<String, String>>,
}

#[derive(Debug, Clone)]
enum Message {
    ServerUpdate(usize, Result<String, String>),
    AddressChanged(usize, String),
    Tick,
    HistoryUpdated(HistoryEntry),
}

#[derive(Debug, Clone)]
struct Server {
    address: String,
    status:  Status,
}

#[derive(Debug, Clone)]
enum Status {
    Loading,
    Online,
    Error(String),
}

impl Application for App {
    type Executor = executor::Default;
    type Message  = Message;
    type Theme    = iced::Theme;
    type Flags    = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let servers: Vec<_> = ["127.0.0.27:9000", "127.0.0.28:9000", "127.0.0.29:9000"]
            .iter()
            .map(|&a| Server::new(a))
            .collect();

        let commands: Vec<_> = servers.iter()
            .enumerate()
            .map(|(i, s)| check_server(s.address.clone(), i))
            .chain(std::iter::once(Command::perform(tick(), |_| Message::Tick)))
            .collect();

        (Self { servers, history: vec![] }, Command::batch(commands))
    }

    fn title(&self) -> String { "Server Monitor".into() }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ServerUpdate(i, res) => {
                self.servers[i].status = match res {
                    Ok(_)  => Status::Online,
                    Err(e) => Status::Error(e),
                };
                Command::none()
            }
            Message::AddressChanged(i, text) => {
                self.servers[i].address = text;
                Command::none()
            }
            Message::Tick => {
                let addresses = self.servers.iter().map(|s| s.address.clone()).collect();
                Command::batch(vec![
                    Command::perform(tick(), |_| Message::Tick),
                    Command::perform(check_all(addresses), Message::HistoryUpdated)
                ])
            }
            Message::HistoryUpdated(entry) => {
                self.history.push(entry);
                if self.history.len() > 20 { self.history.remove(0); }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let server_view = self.servers.iter()
            .enumerate()
            .fold(Column::new(), |col, (i, s)| col.push(s.view(i)));

        let history_view = self.history.iter()
            .fold(Column::new(), |col, e| col.push(history_row(e)));

        Container::new(Column::new()
            .push(header_row(&["Server Address", "Status"]))
            .push(Scrollable::new(server_view).height(Length::FillPortion(2)))
            .push(Text::new("Request History").size(20))
            .push(header_row(&["Time", "Responses"]))
            .push(Scrollable::new(history_view).height(Length::FillPortion(2)))
        ).padding(20).into()
    }
}

impl Server {
    fn new(address: impl Into<String>) -> Self {
        Self { address: address.into(), status: Status::Loading }
    }

    fn view(&self, index: usize) -> Element<Message> {
        let status = match &self.status {
            Status::Loading  => Text::new("Loading...").style(TEXT_GRAY),
            Status::Online   => Text::new("Online").style(TEXT_GREEN),
            Status::Error(e) => Text::new(e).style(TEXT_RED),
        };

        Row::new()
            .push(input_field(&self.address, index))
            .push(status.width(HALF_WIDTH))
            .padding(10)
            .spacing(20)
            .into()
    }
}

fn header_row<'a>(items: &[&'a str]) -> Row<'a, Message> {
    items.iter()
        .fold(Row::new().padding(10), |row, &text| 
            row.push(Text::new(text).width(HALF_WIDTH))
        )
}

fn history_row(entry: &HistoryEntry) -> Row<Message> {
    let time = entry.timestamp.with_timezone(&Local).format("%T").to_string();
    let cells = entry.responses.iter().map(|res| 
        Text::new(match res {
            Ok(d)  => format!("✓ {d}"),
            Err(e) => format!("✗ {e}"),
        }).width(HALF_WIDTH).into()
    );

    Row::new()
        .push(Text::new(time).width(HALF_WIDTH))
        .push(Row::with_children(cells).spacing(10))
        .padding(10)
}

fn input_field(value: &str, index: usize) -> iced::widget::TextInput<'_, Message> {
    text_input("Server address", value)
        .on_input(move |t| Message::AddressChanged(index, t))
        .on_submit(Message::Tick)
        .width(HALF_WIDTH)
}

async fn check_server_task(address: String) -> Result<String, String> {
    let mut stream = TcpStream::connect(&address).await
        .map_err(|e| format!("Connect failed: {e}"))?;

    stream.write_all(b"getData").await
        .map_err(|e| format!("Write failed: {e}"))?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await
        .map_err(|e| format!("Read failed: {e}"))?;

    String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8: {e}"))
}

async fn check_all(addresses: Vec<String>) -> HistoryEntry {
    let responses = futures::future::join_all(
        addresses.into_iter().map(check_server_task)
    ).await;

    HistoryEntry { timestamp: Utc::now(), responses }
}

async fn tick() { sleep(Duration::from_secs(5)).await }

fn check_server(address: String, index: usize) -> Command<Message> {
    Command::perform(check_server_task(address), move |res| Message::ServerUpdate(index, res))
}

const HALF_WIDTH: Length = Length::FillPortion(1);
const TEXT_GRAY:  theme::Text = theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5));
const TEXT_GREEN: theme::Text = theme::Text::Color(iced::Color::from_rgb(0.0, 0.8, 0.0));
const TEXT_RED:   theme::Text = theme::Text::Color(iced::Color::from_rgb(0.8, 0.0, 0.0));
