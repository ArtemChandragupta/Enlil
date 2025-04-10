use iced::{
    executor, Application, Command, Element, Length,
    widget::{Column, Container, Row, Scrollable, Text, text_input, Canvas, canvas},
    theme, Color, Point, Rectangle
};
use iced::widget::canvas::{Frame, Path, Stroke};
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

struct LineGraph {
    server0_data: Vec<(DateTime<Utc>, f32)>,
    server1_data: Vec<(DateTime<Utc>, f32)>,
}

impl<Message> canvas::Program<Message> for LineGraph {
    type State = ();

    fn draw(&self, _state: &(), renderer: &iced::Renderer, _theme: &iced::Theme, bounds: Rectangle, _cursor: iced::mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        
        if self.server0_data.is_empty() && self.server1_data.is_empty() {
            return vec![frame.into_geometry()];
        }

        let padding = 40.0;
        let width = bounds.width - 2.0 * padding;
        let height = bounds.height - 2.0 * padding;

        // Calculate time range
        let all_times: Vec<DateTime<Utc>> = self.server0_data.iter().map(|(t, _)| *t)
            .chain(self.server1_data.iter().map(|(t, _)| *t))
            .collect();
        
        let min_time = *all_times.iter().min().unwrap_or(&Utc::now());
        let max_time = *all_times.iter().max().unwrap_or(&Utc::now());
        let time_range = (max_time - min_time).num_seconds() as f32;

        // Calculate value range
        let all_values: Vec<f32> = self.server0_data.iter().map(|(_, v)| *v)
            .chain(self.server1_data.iter().map(|(_, v)| *v))
            .collect();
        
        let min_val = all_values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_val = all_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        // Axis drawing
        let axis = Path::new(|p| {
            p.move_to(Point::new(padding, padding));
            p.line_to(Point::new(padding, height + padding));
            p.move_to(Point::new(padding, height + padding));
            p.line_to(Point::new(width + padding, height + padding));
        });
        frame.stroke(&axis, Stroke::default().with_width(2.0));

        // Data plotting
        let mut plot_data = |data: &[(DateTime<Utc>, f32)], color: Color| {
            if data.len() < 2 { return; }

            let mut path = Path::new(|p| {
                p.move_to(self.to_point(data[0], min_time, time_range, min_val, max_val, padding, width, height));
                for point in &data[1..] {
                    p.line_to(self.to_point(*point, min_time, time_range, min_val, max_val, padding, width, height));
                }
            });

            frame.stroke(&path, Stroke::default().with_width(2.0).with_color(color));

            for &(t, v) in data {
                let point = self.to_point((t, v), min_time, time_range, min_val, max_val, padding, width, height);
                frame.fill(&Path::circle(point, 3.0), color);
            }
        };

        plot_data(&self.server0_data, Color::from_rgb(0.0, 0.8, 0.0));
        plot_data(&self.server1_data, Color::from_rgb(0.0, 0.0, 0.8));

        vec![frame.into_geometry()]
    }
}

impl LineGraph {
    fn to_point(&self, (t, v): (DateTime<Utc>, f32), min_time: DateTime<Utc>, time_range: f32, min_val: f32, max_val: f32, padding: f32, width: f32, height: f32) -> Point {
        let x = padding + ((t - min_time).num_seconds() as f32 / time_range.max(1.0)) * width;
        let y = padding + height - ((v - min_val) / (max_val - min_val).max(1.0)) * height;
        Point::new(x, y)
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = ();

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
            Message::ServerUpdate(i, res) => {
                self.servers[i].status = match res {
                    Ok(_)  => Status::Online,
                    Err(e) => Status::Error(e),
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

        let server0_data: Vec<_> = self.history.iter()
            .filter_map(|e| e.responses.get(0)
                .and_then(|r| r.as_ref().ok())
                .and_then(|s| s.parse().ok())
                .map(|v| (e.timestamp, v))
            ).collect();

        let server1_data: Vec<_> = self.history.iter()
            .filter_map(|e| e.responses.get(1)
                .and_then(|r| r.as_ref().ok())
                .and_then(|s| s.parse().ok())
                .map(|v| (e.timestamp, v))
            ).collect();

        let graph = Canvas::new(LineGraph { server0_data, server1_data })
            .width(Length::Fill)
            .height(Length::Fill);
            // .height(Length::Units(300));

        Container::new(Column::new()
            .push(header_row(&["Server Address", "Status"]))
            .push(Scrollable::new(server_view).height(Length::FillPortion(2)))
            .push(Text::new("Request History").size(20))
            .push(header_row(&["Time", "Responses"]))
            .push(Scrollable::new(history_view).height(Length::FillPortion(2)))
            .push(Text::new("Performance Graph").size(20))
            .push(graph)
        ).padding(20).into()
    }
}

impl Server {
    fn new(address: impl Into<String>) -> Self {
        Self { address: address.into(), status: Status::Loading }
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
