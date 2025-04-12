use iced::{
    executor, widget::{button, column, container, row, scrollable, text, text_input}, 
    Application, Command, Element, Length, Theme
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream
};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct App {
    servers: Vec<Server>,
    history: History,
}

#[derive(Debug, Clone)]
struct Server {
    id: Uuid,
    address: String,
    status: Status,
}

#[derive(Debug, Clone)]
struct History {
    entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    timestamp: chrono::DateTime<Utc>,
    responses: Vec<(Uuid, Response)>,
}

#[derive(Debug, Clone)]
enum Status {
    Loading,
    Online,
    Error(String),
}

#[derive(Debug, Clone)]
enum Response {
    Success(String),
    Error(String),
}

#[derive(Debug, Clone)]
enum Message {
    AddressChanged(Uuid, String),
    RefreshPressed,
    CheckCompleted(HistoryEntry),
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app = App {
            servers: (1..=3)
                .map(|i| Server::new(format!("127.0.0.{}:9000", 26 + i)))
                .collect(),
            history: History::new(),
        };
        
        (app.clone(), Command::perform(check_servers(app.servers.clone()), Message::CheckCompleted))
    }

    fn title(&self) -> String { "Server Monitor".into() }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::AddressChanged(id, addr) => {
                if let Some(server) = self.servers.iter_mut().find(|s| s.id == id) {
                    server.address = addr;
                }
                Command::none()
            }
            
            Message::RefreshPressed => Command::perform(
                check_servers(self.servers.clone()),
                Message::CheckCompleted
            ),
            
            Message::CheckCompleted(entry) => {
                self.history.add(entry.clone());
                
                for (id, response) in entry.responses {
                    if let Some(server) = self.servers.iter_mut().find(|s| s.id == id) {
                        server.status = match response {
                            Response::Success(_) => Status::Online,
                            Response::Error(e) => Status::Error(e),
                        };
                    }
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        column![
            controls(),
            scrollable(column(self.servers.iter().map(Server::view)))
                .height(Length::FillPortion(2)),
            history_view(&self.history)
        ]
        .padding(20)
        .into()
    }
}

impl Server {
    fn new(address: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            address: address.into(),
            status: Status::Loading,
        }
    }

    fn view(&self) -> Element<Message> {
        row![
            text_input("Server address", &self.address)
                .on_input(move |s| Message::AddressChanged(self.id, s))
                .width(Length::FillPortion(2)),
            container(match &self.status {
                Status::Loading  => text("⏳ Loading...").  style(iced::Color::from_rgb(0.5, 0.5, 0.5)),
                Status::Online   => text("✅ Online").      style(iced::Color::from_rgb(0.0, 0.8, 0.0)),
                Status::Error(e) => text(format!("❌ {e}")).style(iced::Color::from_rgb(0.8, 0.0, 0.0)),
            }).width(Length::FillPortion(1))
        ]
        .spacing(10)
        .padding(5)
        .into()
    }
}

impl History {
    fn new() -> Self {
        Self { entries: Vec::with_capacity(10) }
    }

    fn add(&mut self, entry: HistoryEntry) {
        if self.entries.len() >= 10 {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }
}

fn controls() -> Element<'static, Message> {
    row![
        text("Server Monitor").size(24),
        button("⟳ Refresh").on_press(Message::RefreshPressed)
    ]
    .spacing(20)
    .into()
}

fn history_view(history: &History) -> Element<Message> {
    column![
        text("History").size(20),
        scrollable(
            column(history.entries.iter().map(|e| 
                text(format!("{}: {} checks", e.timestamp, e.responses.len())).into()
            ).collect::<Vec<_>>())
        )
    ].into()
}

async fn check_server(address: String) -> Response {
    match TcpStream::connect(&address).await {
        Ok(mut stream) => {
            let result: Result<String, Box<dyn std::error::Error>> = async {
                stream.write_all(b"getData").await?;
                let mut buf = Vec::new();
                stream.read_to_end(&mut buf).await?;
                String::from_utf8(buf).map_err(|e| e.into())
            }.await;

            match result {
                Ok(data) => Response::Success(data),
                Err(e)   => Response::Error(e.to_string()),
            }
        }
        Err(e) => Response::Error(e.to_string()),
    }
}

async fn check_servers(servers: Vec<Server>) -> HistoryEntry {
    let responses = futures::future::join_all(
        servers.iter().map(|server| async move {
            let response = check_server(server.address.clone()).await;
            (server.id, response)
        })
    ).await;

    HistoryEntry {
        timestamp: Utc::now(),
        responses,
    }
}

fn main() -> iced::Result {
    App::run(iced::Settings::default())
}
