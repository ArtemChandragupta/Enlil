use iced::{
    executor, Application, Command, Element, Length,
    widget::{Column, Container, Row, Scrollable, Text, text_input},
    theme
};
use plotters_iced::{Chart, ChartWidget, DrawingBackend};
use plotters::prelude::*;
use plotters::style::Color;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, time::{sleep, Duration}};
use chrono::{DateTime, Local, Utc};
use std::collections::VecDeque;

fn main() -> iced::Result {
    App::run(iced::Settings::default())
}

struct App {
    servers: Vec<Server>,
    history: VecDeque<HistoryEntry>,
    chart_data: ChartData,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    timestamp: DateTime<Utc>,
    responses: Vec<Result<String, String>>,
}

#[derive(Debug, Clone)]
struct ChartData {
    server27_data: Vec<(f64, f64)>,
    server28_data: Vec<(f64, f64)>,
    timestamps: Vec<DateTime<Utc>>,
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

struct LineChart;

impl Chart<Message> for LineChart {
    type State = ();

    // fn build_chart<DB: DrawingBackend>(
    //     &self,
    //     state: &Self::State,
    //     chart: &mut ChartBuilder<DB>,
    //     _bounds: iced::Rectangle,
    // ) {
    //     let mut chart = chart
    //         .caption("Server Performance", ("sans-serif", 20))
    //         .x_label_area_size(30)
    //         .y_label_area_size(40)
    //         .margin(20)
    //         .build_cartesian_2d(0f64..20f64, 0f64..100f64)
    //         .unwrap();
    //
    //     chart
    //         .configure_mesh()
    //         .x_labels(5)
    //         .y_labels(5)
    //         .x_desc("Time")
    //         .y_desc("Value")
    //         .draw()
    //         .unwrap();
    // }

    fn build_chart<DB: DrawingBackend>(
        &self,
        _state: &Self::State,
        chart: &mut ChartBuilder<DB>,
        _bounds: iced::Rectangle,
    ) {
        // Перенесли логику из метода draw сюда
        let y_range = self.server27_data.iter().chain(&self.server28_data)
            .map(|(_, y)| *y)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), y| 
                (min.min(y), max.max(y)));
        
        let mut chart = chart
            .caption("Server Performance", ("sans-serif", 20))
            .margin(20)
            .build_cartesian_2d(0f64..20f64, y_range.0..y_range.1)
            .unwrap();
            
        // Остальная логика отрисовки...
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

        (Self { 
            servers, 
            history: VecDeque::with_capacity(20),
            chart_data: ChartData {
                server27_data: Vec::new(),
                server28_data: Vec::new(),
                timestamps: Vec::new(),
            }
        }, Command::batch(commands))
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
                // Обновляем историю
                if self.history.len() >= 20 {
                    self.history.pop_front();
                }
                self.history.push_back(entry.clone());

                // Обновляем данные для графика
                if let (Some(Ok(val27)), Some(Ok(val28))) = (
                    entry.responses.get(0).and_then(|r| r.as_ref().ok()).and_then(|s| s.parse().ok()),
                    entry.responses.get(1).and_then(|r| r.as_ref().ok()).and_then(|s| s.parse().ok()),
                ) {
                    self.chart_data.timestamps.push(entry.timestamp);
                    self.chart_data.server27_data.push((
                        self.chart_data.server27_data.len() as f64,
                        val27
                    ));
                    self.chart_data.server28_data.push((
                        self.chart_data.server28_data.len() as f64,
                        val28
                    ));

                    // Ограничиваем до 20 точек
                    if self.chart_data.server27_data.len() > 20 {
                        self.chart_data.server27_data.remove(0);
                        self.chart_data.server28_data.remove(0);
                        self.chart_data.timestamps.remove(0);
                    }
                }
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

        let chart = ChartWidget::new(&self.chart_data)
            .width(Length::Fill)
            .height(Length::Units(300));

        Container::new(Column::new()
            .push(header_row(&["Server Address", "Status"]))
            .push(Scrollable::new(server_view).height(Length::FillPortion(2)))
            .push(Text::new("Request History").size(20))
            .push(header_row(&["Time", "Responses"]))
            .push(Scrollable::new(history_view).height(Length::FillPortion(2)))
            .push(Text::new("Performance Graph").size(20))
            .push(chart)
        ).padding(20).into()
    }
}

impl ChartData {
    fn draw(&self, backend: &mut DrawingBackend) {
        let root = backend.draw().unwrap();
        let root = root.titled("Server Performance", ("sans-serif", 20)).unwrap();

        let (x_min, x_max) = (0.0, 20.0);
        let y_range = self.server27_data.iter().chain(&self.server28_data)
            .map(|(_, y)| *y)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), y| 
                (min.min(y), max.max(y)))
            .1;

        let mut chart = ChartBuilder::on(&root)
            .margin(20)
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(x_min..x_max, 0.0..y_range)
            .unwrap();

        chart.configure_mesh()
            .x_labels(5)
            .y_labels(5)
            .x_desc("Time Index")
            .y_desc("Value")
            .draw()
            .unwrap();

        // Рисуем линии
        chart.draw_series(LineSeries::new(
            self.server27_data.iter().map(|(x, y)| (*x, *y)),
            &RED,
        )).unwrap()
        .label("Server 127.0.0.27:9000")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

        chart.draw_series(LineSeries::new(
            self.server28_data.iter().map(|(x, y)| (*x, *y)),
            &BLUE,
        )).unwrap()
        .label("Server 127.0.0.28:9000")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()
            .unwrap();
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
