use iced::{
    executor, Application, Command, Element, Length,
    widget::{Column, Container, Row, Scrollable, Text, text_input},
    theme,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, Duration},
};
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
    ServerUpdated(usize, Result<String, String>),
    ServerAddressInputChanged(usize, String),
    ServerAddressSubmitted(usize),
    Tick,
    CheckAllServersComplete(HistoryEntry),
}

#[derive(Debug, Clone)]
struct Server {
    input_address: String,
    address: String,
    status: Status,
}

#[derive(Debug, Clone)]
enum Status {
    Loading,
    Online(()),
    Error(String),
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let servers: Vec<_> = vec![
            "127.0.0.27:9000",
            "127.0.0.28:9000",
            "127.0.0.203:9000",
            "127.0.0.204:9000",
        ].into_iter()
            .map(|addr| Server::with_address(addr.to_string()))
            .collect();

        let initial_commands: Vec<_> = servers.iter()
            .enumerate()
            .map(|(i, s)| check_server(s.address.clone(), i))
            .collect();

        let timer_command = Command::perform(async { sleep(Duration::from_secs(5)).await }, |_| Message::Tick);

        (
            Self { servers, history: Vec::new() },
            Command::batch(initial_commands.into_iter().chain(Some(timer_command)))
        )
    }

    fn title(&self) -> String {
        "Server Monitor".into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ServerUpdated(index, result) => {
                self.servers[index].status = match result {
                    Ok(..) => Status::Online(()),
                    Err(e) => Status::Error(e),
                };
                
                let address = self.servers[index].address.clone();
                check_server(address, index)
            }
            Message::ServerAddressInputChanged(index, text) => {
                self.servers[index].input_address = text;
                Command::none()
            }
            Message::ServerAddressSubmitted(index) => {
                let server = &mut self.servers[index];
                server.address = server.input_address.clone();
                server.status = Status::Loading;
                let address = server.address.clone();
                check_server(address, index)
            }
            Message::Tick => {
                let next_tick = Command::perform(
                    async { sleep(Duration::from_secs(5)).await },
                    |_| Message::Tick
                );

                let addresses: Vec<String> = self.servers.iter().map(|s| s.address.clone()).collect();
                let check_command = Command::perform(
                    check_all_servers(addresses),Message::CheckAllServersComplete
                );

                Command::batch(vec![next_tick, check_command])
            }
            Message::CheckAllServersComplete(entry) => {
                self.history.push(entry);
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let servers_header = Row::new()
            .push(cell("Server Address".to_string()).width(Length::FillPortion(1)))
            .push(cell("Status".to_string()).width(Length::FillPortion(1)))
            .padding(10);

        let servers_list = Column::with_children(
            self.servers.iter()
                .enumerate()
                .map(|(index, server)| server.view(index))
                // .collect()
        );

        let history_header = Row::new()
            .push(cell("Time".to_string()).width(Length::FillPortion(1)))
            .push(
                Row::with_children(
                    self.servers.iter()
                        .map(|s| cell(s.address.clone()).width(Length::FillPortion(1)).into())
                )
                .width(Length::FillPortion(4))
                .spacing(10)
            )
            .padding(10)
            .spacing(20);

        let recent_history = if self.history.len() > 20 {
            &self.history[self.history.len() - 20..]
        } else {
            &self.history[..]
        };

        let history_rows = recent_history.iter().map(|entry| {
            let datetime: DateTime<Local> = DateTime::from(entry.timestamp);
            let time_str = datetime.format("%H:%M:%S").to_string();
            let time_cell = cell(time_str).width(Length::FillPortion(1));

            let response_cells = entry.responses.iter().map(|res| {
                let text = match res {
                    Ok(data) => format!("OK: {}", data),
                    Err(e) => format!("ERR: {}", e),
                };
                Text::new(text).width(Length::FillPortion(1)).into()
            });

            Row::new()
                .push(time_cell)
                .push(
                    Row::with_children(response_cells)
                        .width(Length::FillPortion(4))
                        .spacing(10)
                )
                .padding(10)
                .spacing(20)
                .into()
        });

        Container::new(
            Column::new()
                .push(servers_header)
                .push(Scrollable::new(servers_list).height(Length::FillPortion(2)))
                .push(Text::new("Request History").size(20))
                .push(history_header)
                .push(Scrollable::new(Column::with_children(history_rows)).height(Length::FillPortion(2)))
        )
        .padding(20)
        .into()
    }
}

impl Server {
    fn with_address(address: String) -> Self {
        Self {
            input_address: address.clone(),
            address,
            status: Status::Loading,
        }
    }

    fn view(&self, index: usize) -> Element<Message> {
        let status_text = match &self.status {
            Status::Loading => Text::new("Loading...")
                .style(theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5))),
            Status::Online(_) => Text::new("Online")
                .style(theme::Text::Color(iced::Color::from_rgb(0.0, 0.8, 0.0))),
            Status::Error(e) => Text::new(e.clone())
                .style(theme::Text::Color(iced::Color::from_rgb(0.8, 0.0, 0.0))),
        };

        let address_input = text_input("Server address", &self.input_address)
            .on_input(move |text| Message::ServerAddressInputChanged(index, text))
            .on_submit(Message::ServerAddressSubmitted(index))
            .width(Length::FillPortion(1));

        Row::new()
            .push(address_input)
            .push(status_text.width(Length::FillPortion(1)))
            .padding(10)
            .spacing(20)
            .into()
    }
}

fn cell(content: impl Into<String>) -> Text<'static> {
    Text::new(content.into())
}

async fn check_server_task(address: String) -> Result<String, String> {
    let mut stream = match TcpStream::connect(&address).await {
        Ok(stream) => stream,
        Err(e) => return Err(format!("Connection failed: {e}")),
    };

    if let Err(e) = stream.write_all(b"getData").await {
        return Err(format!("Write failed: {e}"));
    }

    let mut buf = Vec::new();
    if let Err(e) = stream.read_to_end(&mut buf).await {
        return Err(format!("Read failed: {e}"));
    }

    String::from_utf8(buf)
        .map_err(|e| format!("Invalid response: {e}"))
}

async fn check_all_servers(addresses: Vec<String>) -> HistoryEntry {
    let results = futures::future::join_all(
        addresses.into_iter().map(check_server_task)
    ).await;

    HistoryEntry {
        timestamp: Utc::now(),
        responses: results,
    }
}

fn check_server(address: String, index: usize) -> Command<Message> {
    Command::perform(
        async move {
            sleep(Duration::from_secs(5)).await;
            check_server_task(address).await
        },
        move |result| Message::ServerUpdated(index, result)
    )
}
