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

fn main() -> iced::Result {
    App::run(iced::Settings::default())
}

struct App {
    servers: Vec<Server>,
}

#[derive(Debug, Clone)]
enum Message {
    ServerUpdated(usize, Result<String, String>),
    ServerAddressInputChanged(usize, String),
    ServerAddressSubmitted(usize),
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
    Online(String),
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

        let commands: Vec<_> = servers.iter()
            .enumerate()
            .map(|(i, s)| check_server(s.address.clone(), i))
            .collect();

        (Self { servers }, Command::batch(commands))
    }

    fn title(&self) -> String {
        "Server Monitor".into()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ServerUpdated(index, result) => {
                self.servers[index].status = match result {
                    Ok(data) => Status::Online(data),
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
        }
    }

    fn view(&self) -> Element<Message> {
        // Шапка для списка серверов
        let servers_header = Row::new()
            .push(cell("Server Address").width(Length::FillPortion(1)))
            .push(cell("Status").width(Length::FillPortion(1)))
            .padding(10);

        // Список серверов
        let servers_list = Column::with_children(
            self.servers.iter()
                .enumerate()
                .map(|(index, server)| server.view(index))
                // .collect()
        );

        // Собираем данные для таблицы
        let table_data: Vec<(&String, &String)> = self.servers.iter()
            .filter_map(|server| {
                if let Status::Online(data) = &server.status {
                    Some((&server.address, data))
                } else {
                    None
                }
            })
            .collect();

        // Шапка таблицы данных
        let data_header = Row::new()
            .push(cell("Server Address").width(Length::FillPortion(1)))
            .push(cell("Received Data").width(Length::FillPortion(1)))
            .padding(10);

        // Тело таблицы данных
        let data_rows = table_data.iter().map(|(address, data)| {
            Row::new()
                .push(cell(address).width(Length::FillPortion(1)))
                .push(cell(data).width(Length::FillPortion(1)))
                .padding(10)
                .into()
        });

        // Основной контейнер
        Container::new(
            Column::new()
                .push(servers_header)
                .push(Scrollable::new(servers_list).height(Length::FillPortion(2)))
                .push(Text::new("Data from Online Servers").size(20).width(Length::Fill))
                .push(data_header)
                .push(Scrollable::new(Column::with_children(data_rows)).height(Length::FillPortion(2)))
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
            Status::Error(e) => Text::new(e)
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

fn cell(content: &str) -> Text {
    Text::new(content)
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

fn check_server(address: String, index: usize) -> Command<Message> {
    Command::perform(
        async move {
            sleep(Duration::from_secs(5)).await;
            check_server_task(address).await
        },
        move |result| Message::ServerUpdated(index, result)
    )
}
