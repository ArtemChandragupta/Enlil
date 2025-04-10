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
    is_checking: bool,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    timestamp: DateTime<Utc>,
    responses: Vec<Result<String, String>>,
}

#[derive(Debug, Clone)]
enum Message {
    BatchUpdate(Vec<(usize, Result<String, String>)>, HistoryEntry),
    AddressChanged(usize, String),
    Tick,
    ManualCheck,
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
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let servers = ["127.0.0.27:9000", "127.0.0.28:9000", "127.0.0.203:9000", "127.0.0.204:9000"]
            .iter()
            // .enumerate()
            .map(|&a| Server {
                address: a.into(),
                status: Status::Loading,
            })
            .collect();

        (Self { 
            servers, 
            history: Vec::with_capacity(20),
            is_checking: false,
        }, Command::perform(tick(), |_| Message::Tick))
    }

    fn title(&self) -> String { "Server Monitor".into() }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::BatchUpdate(updates, entry) => {
                self.is_checking = false;
                updates.iter().for_each(|(i, res)| {
                    self.servers[*i].status = match res {
                        Ok(_) => Status::Online,
                        Err(e) => Status::Error(e.clone()),
                    };
                });

                self.history.push(entry);
                if self.history.len() > 20 {
                    self.history.remove(0);
                }
                
                Command::none()
            }
            
            Message::AddressChanged(i, text) => {
                self.servers[i].address = text;
                Command::none()
            }

            Message::ManualCheck => self.check_servers_command(),

            Message::Tick => {
                if self.is_checking {
                    Command::none()
                } else {
                    self.is_checking = true;
                    Command::batch(vec![
                        Command::perform(tick(), |_| Message::Tick),
                        self.check_servers_command()
                    ])
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        Container::new(
            Column::new()
                .push(header_row(&["Server Address", "Status"]))
                .push(Scrollable::new(
                    Column::with_children(
                        self.servers.iter()
                            .enumerate()
                            .map(|(i, s)| s.view(i))
                            .collect::<Vec<_>>()
                    )
                    .width(Length::Fill)
                ).height(Length::FillPortion(2)))
                .push(Text::new("Request History").size(20))
                .push(header_row(&["Time", "Responses"]))
                .push(Scrollable::new(
                    Column::with_children(
                        self.history.iter()
                        .map(history_row)
                        .collect::<Vec<_>>()
                ).height(Length::FillPortion(2))))
        ).padding(20).into()
    }
}

impl App {
    fn server_addresses(&self) -> Vec<(usize, String)> {
        self.servers.iter()
            .enumerate()
            .map(|(i, s)| (i, s.address.clone()))
            .collect()
    }

    fn check_servers_command(&self) -> Command<Message> {
        Command::perform(
            check_servers(self.server_addresses()),
            |(updates, entry)| Message::BatchUpdate(updates, entry)
        )
    }
}

fn input_field(value: &str, index: usize) -> iced::widget::TextInput<'_, Message> {
    text_input("Server address", value)
        .on_input(move |t| Message::AddressChanged(index, t))
        .on_submit(Message::ManualCheck)
}

impl Server {
    fn view(&self, index: usize) -> Element<Message> {
        let status = match &self.status {
            Status::Loading => Text::new("Loading...").style(TEXT_GRAY),
            Status::Online => Text::new("Online").style(TEXT_GREEN),
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

fn header_row<'a>(headers: &[&'a str]) -> Row<'a, Message> {
    Row::with_children(
        headers.iter()
            .map(|&text| Text::new(text).width(HALF_WIDTH).into())
            // .collect()
    ).padding(10)
}

// fn history_row(entry: &HistoryEntry) -> Row<Message> {
//     Row::new()
//         .push(Text::new(entry.timestamp.with_timezone(&Local).format("%T").to_string()).width(HALF_WIDTH))
//         .push(Row::with_children(
//             entry.responses.iter().map(|res| match res {
//                 Ok(d) => Text::new(format!("✓ {d}")).style(TEXT_GREEN),
//                 Err(e) => Text::new(format!("✗ {e}")).style(TEXT_RED),
//             }.width(HALF_WIDTH).into())
//         ).spacing(10))
//         .padding(10)
//         .into()
// }

fn history_row(entry: &HistoryEntry) -> Element<Message> {
    Row::new()
        .push(
            Text::new(entry.timestamp.with_timezone(&Local).format("%T").to_string())
                .width(HALF_WIDTH)
        )
        .push(
            Row::with_children(
                entry.responses.iter().map(|res| {
                    let text = match res {
                        Ok(d) => Text::new(format!("✓ {d}")).style(TEXT_GREEN),
                        Err(e) => Text::new(format!("✗ {e}")).style(TEXT_RED),
                    };
                    text.width(HALF_WIDTH).into()
                })
                .collect::<Vec<_>>()
            )
            .spacing(10)
        )
        .padding(10)
        .into()
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

async fn check_servers(servers: Vec<(usize, String)>) -> (Vec<(usize, Result<String, String>)>, HistoryEntry) {
    let results = futures::future::join_all(
        servers.into_iter().map(|(i, address)| async move {
            (i, check_server_task(address).await)
        })
    ).await;

    let entry = HistoryEntry {
        timestamp: Utc::now(),
        responses: results.iter().map(|(_, res)| res.clone()).collect(),
    };
    
    (results, entry)
}

async fn tick() { sleep(Duration::from_secs(5)).await }

const HALF_WIDTH: Length = Length::FillPortion(1);
const TEXT_GRAY: theme::Text = theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5));
const TEXT_GREEN: theme::Text = theme::Text::Color(iced::Color::from_rgb(0.0, 0.8, 0.0));
const TEXT_RED: theme::Text = theme::Text::Color(iced::Color::from_rgb(0.8, 0.0, 0.0));
