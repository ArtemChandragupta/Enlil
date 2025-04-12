use iced::{
    executor, Application, Command, Element, Length,
    widget::{Column, Container, Row, Scrollable, Text, text_input},
    theme,
};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, time::Duration};
use chrono::{DateTime, Local, Utc};


// Main App structure
struct App {
    servers: Vec<Server>,
    history: Vec<HistoryEntry>,
}


// Model (state) components
#[derive(Debug, Clone)]
struct HistoryEntry {
    timestamp: DateTime<Utc>,
    responses: Vec<Result<String, String>>,
}

#[derive(Debug, Clone)]
struct Server {
    address: String,
    status:  Status,
}


// Status type
#[derive(Debug, Clone)]
enum Status {
    Loading,
    Online,
    Error(String),
}


// Messages
#[derive(Debug, Clone)]
enum Message {
    BatchUpdate(Vec<(usize, Result<String, String>)>, HistoryEntry),
    AddressChanged(usize, String),
}


// Application implementation
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

        let initial_addresses = current_addresses(&servers);

        // Запускаем первую проверку сразу
        let command = Command::perform(
            delayed_check(initial_addresses),
            |(updates, entry)| Message::BatchUpdate(updates, entry)
        );

        (Self { servers, history: vec![] }, command)
    }

    fn title(&self) -> String { "Server Monitor".into() }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::BatchUpdate(updates, entry) => {
                for (i, res) in &updates {
                    self.servers[*i].status = match res {
                        Ok(_)  => Status::Online,
                        Err(e) => Status::Error(e.clone()),
                    };
                }

                add_history_entry(&mut self.history, entry);

                let next_addresses = current_addresses(&self.servers);

                Command::perform(
                    delayed_check(next_addresses),
                    |(updates, entry)| Message::BatchUpdate(updates, entry)
                )
            }
            
            Message::AddressChanged(i, text) => {
                self.servers[i].address = text;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let server_views = self.servers.iter()
            .enumerate()
            .map(|(i, s)| server_view(i, &s.address, &s.status))
            .collect::<Vec<_>>();

        let history_view = self.history.iter()
            .fold(Column::new(), |col, e| col.push(history_row(e)));

        Container::new(Column::new()
            .push(header_row(&["Server Address", "Status"]))
            .push(Scrollable::new(Column::with_children(server_views)))
            .push(Text::new("Request History").size(20))
            .push(header_row(&["Time", "Responses"]))
            .push(Scrollable::new(history_view).height(Length::FillPortion(2)))
        ).padding(20).into()
    }
}


// Server Implementation
impl Server {
    fn new(address: impl Into<String>) -> Self {
        Self { address: address.into(), status: Status::Loading }
    }
}


// Helper functions
fn add_history_entry(history: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    history.push(entry);
    if history.len() > 10 {
        history.remove(0);
    }
}


// View Components
fn header_row(items: &[&'static str]) -> Row<'static, Message> {
    items.iter()
        .fold(Row::new().padding(10), |row, &text| 
            row.push(Text::new(text).width(HALF_WIDTH))
        )
}

fn history_row(entry: &HistoryEntry) -> Row<Message> {
    let time  = entry.timestamp.with_timezone(&Local).format("%T").to_string();
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

// Компонент: Строка сервера
fn server_view<'a>(
    index: usize,
    address: &'a str,
    status: &'a Status,
) -> Element<'a, Message> {
    fn address_input(index: usize, address: &str) -> iced::widget::TextInput<Message> {
        text_input("Server address", address)
            .on_input(move |text| Message::AddressChanged(index, text))
            .width(HALF_WIDTH)
    }

    fn status_text(status: &Status) -> Text {
        let (text, style) = match status {
            Status::Loading  => ("Loading...", TEXT_GRAY ),
            Status::Online   => ("Online",     TEXT_GREEN),
            Status::Error(e) => (e.as_str(),   TEXT_RED  ),
        };
        
        Text::new(text).style(style).width(HALF_WIDTH)
    }

    Row::new()
        .push(address_input(index, address))
        .push(status_text(status))
        .padding(10)
        .spacing(20)
        .into()
}


// Network Operations
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
            let result = check_server_task(address).await;
            (i, result)
        })
    ).await;

    let entry = HistoryEntry {
        timestamp: Utc::now(),
        responses: results.iter().map(|(_, res)| res.clone()).collect(),
    };
    
    (results, entry)
}

async fn delayed_check(servers: Vec<(usize, String)>) -> (Vec<(usize, Result<String, String>)>, HistoryEntry) {
    tokio::time::sleep(Duration::from_secs(2)).await;
    check_servers(servers).await
}

fn current_addresses(servers: &[Server]) -> Vec<(usize, String)> {
    servers.iter()
        .enumerate()
        .map(|(i, s)| (i, s.address.clone()))
        .collect()
}


// Constants
const HALF_WIDTH: Length = Length::FillPortion(1);
const TEXT_GRAY:  theme::Text = theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.5));
const TEXT_GREEN: theme::Text = theme::Text::Color(iced::Color::from_rgb(0.0, 0.8, 0.0));
const TEXT_RED:   theme::Text = theme::Text::Color(iced::Color::from_rgb(0.8, 0.0, 0.0));

fn main() -> iced::Result {
    App::run(iced::Settings::default())
}
