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
    last_value: Option<String>,
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
        let servers: Vec<_> = ["127.0.0.27:9000", "127.0.0.28:9000", "127.0.0.203:9000", "127.0.0.204:9000"]
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
            // Message::ServerUpdate(i, res) => {
            //     self.servers[i].status = match res {
            //         Ok(_)  => Status::Online,
            //         Err(e) => Status::Error(e),
            //     };
            //     check_server(self.servers[i].address.clone(), i)
            // }
            Message::ServerUpdate(i, res) => {
                match &res {
                    Ok(data) => {
                        self.servers[i].last_value = Some(data.clone());
                        self.servers[i].status = Status::Online;
                    },
                    Err(e)   => {
                        self.servers[i].status = Status::Error(e.clone());
                    }
                };
                check_server(self.servers[i].address.clone(), i)
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
            // Message::HistoryUpdated(entry) => {
            //     self.history.push(entry);
            //     if self.history.len() > 20 { self.history.remove(0); }
            //     Command::none()
            // }
            Message::HistoryUpdated(entry) => {
                // Модифицируем ответы, заменяя ошибки на последние успешные значения или "0"
                let mut modified_responses = Vec::new();
                for (i, res) in entry.responses.into_iter().enumerate() {
                    let modified_res = match res {
                        Ok(data) => Ok(data),
                        Err(_) => {
                            let value = self.servers.get(i)
                                .and_then(|s| s.last_value.clone())
                                .unwrap_or_else(|| "0".to_string());
                            Ok(value)
                        }
                    };
                    modified_responses.push(modified_res);
                }
                let modified_entry = HistoryEntry {
                    timestamp: entry.timestamp,
                    responses: modified_responses,
                };
                self.history.push(modified_entry);
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
        Self { address: address.into(), status: Status::Loading, last_value: None }
    }

    fn view(&self, index: usize) -> Element<Message> {
        let status = match &self.status {
            Status::Loading  => Text::new("Loading...").style(gray()),
            Status::Online   => Text::new("Online").style(green()),
            Status::Error(e) => Text::new(e).style(red()),
        };

        Row::new()
            .push(input_field(&self.address, index))
            .push(status.width(half_width()))
            .padding(10)
            .spacing(20)
            .into()
    }
}

fn header_row<'a>(items: &[&'a str]) -> Row<'a, Message> {
    items.iter()
        .fold(Row::new().padding(10), |row, &text| 
            row.push(Text::new(text).width(half_width()))
        )
}

fn history_row(entry: &HistoryEntry) -> Row<Message> {
    let time = entry.timestamp.with_timezone(&Local).format("%T").to_string();
    let cells = entry.responses.iter().map(|res| 
        Text::new(match res {
            Ok(d)  => format!("✓ {d}"),
            Err(e) => format!("✗ {e}"),
        }).width(half_width()).into()
    );

    Row::new()
        .push(Text::new(time).width(half_width()))
        .push(Row::with_children(cells).spacing(10))
        .padding(10)
}

fn input_field(value: &str, index: usize) -> iced::widget::TextInput<'_, Message> {
    text_input("Server address", value)
        .on_input(move |t| Message::AddressChanged(index, t))
        .on_submit(Message::Tick)
        .width(half_width())
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

fn half_width() -> Length { Length::FillPortion(1) }
fn gray()  -> theme::Text { theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5)) }
fn green() -> theme::Text { theme::Text::Color(iced::Color::from_rgb(0.0, 0.8, 0.0)) }
fn red()   -> theme::Text { theme::Text::Color(iced::Color::from_rgb(0.8, 0.0, 0.0)) }
