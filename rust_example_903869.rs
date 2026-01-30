// Learning Objective:
// This tutorial demonstrates how to visualize real-time sensor data
// in your terminal using a simple streaming API and a terminal-based graph.
// We'll focus on the core concepts of:
// 1. Receiving data over a network stream.
// 2. Processing that data to create a visual representation.
// 3. Using a library to draw the graph directly in the terminal.

// We'll use the `tokio` runtime for asynchronous operations,
// `reqwest` for making HTTP requests to the streaming API,
// and `tui-rs` (now `ratatui`) for drawing the terminal UI.

// Make sure you have these dependencies in your Cargo.toml:
// [dependencies]
// tokio = { version = "1", features = ["full"] }
// reqwest = { version = "0.11", features = ["stream"] }
// ratatui = "0.25"
// futures = "0.3"

use futures::stream::StreamExt; // For easily working with streams
use ratatui::{
    backend::CrosstermBackend, // Backend for drawing to the terminal
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Text,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
    Frame,
    Terminal,
};
use std::{
    error::Error,
    io::{stdout, Stdout},
    time::Duration,
};

// Define the URL of our simple streaming API.
// In a real-world scenario, this would be your sensor data endpoint.
// For this example, we'll use a placeholder URL.
const SENSOR_API_URL: &str = "http://localhost:8080/sensor/stream"; // Replace with your actual API URL

// Define the maximum number of data points to show on the graph at once.
const MAX_DATA_POINTS: usize = 50;

// A struct to hold our sensor readings.
// This represents a single data point from the sensor.
#[derive(Debug, Clone, PartialEq)]
struct SensorReading {
    timestamp: i64, // Unix timestamp
    value: f64,
}

// A struct to manage our terminal application state.
// This holds the data we want to display and configure the graph.
struct AppState {
    data_points: Vec<(f64, f64)>, // Stores (x, y) coordinates for the graph
    next_x: f64, // Keeps track of the next x-axis value (time)
}

impl AppState {
    // Creates a new, empty application state.
    fn new() -> Self {
        AppState {
            data_points: Vec::new(),
            next_x: 0.0,
        }
    }

    // Adds a new sensor reading to our state.
    // It also ensures we don't exceed the maximum number of data points.
    fn add_reading(&mut self, reading: SensorReading) {
        // We're plotting time on the x-axis and sensor value on the y-axis.
        self.data_points.push((self.next_x, reading.value));
        self.next_x += 1.0; // Increment for the next data point (simulating time progression)

        // If we have too many data points, remove the oldest one.
        if self.data_points.len() > MAX_DATA_POINTS {
            self.data_points.remove(0); // Remove the first element (oldest)
        }
    }
}

// This is the main function that sets up the terminal and starts the data stream.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the terminal.
    // We use CrosstermBackend for interacting with the terminal.
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // Configure terminal behavior for better interactive experience.
    // `enable_raw_mode()` puts the terminal in raw mode, allowing us to capture
    // keystrokes and control the display more granularly.
    // `DisableBracketedPaste::enable()` and `EnableMouseCapture::enable()` are
    // often used with TUI applications for more advanced input handling,
    // though not strictly necessary for this simple example.
    crossterm::terminal::enable_raw_mode()?;
    let mut _restorer = crossterm::execute!(stdout(), crossterm::event::EnableBracketedPaste)?;
    let mut _mouse_capture = crossterm::execute!(stdout(), crossterm::event::EnableMouseCapture)?;


    // Initialize our application state.
    let mut app_state = AppState::new();

    // Create an asynchronous task to fetch sensor data.
    // `tokio::spawn` allows us to run this task concurrently with the terminal rendering.
    let data_fetch_handle = tokio::spawn(async move {
        // Create an HTTP client.
        let client = reqwest::Client::new();

        // Make an asynchronous request to the streaming API.
        // `streaming_request` will yield chunks of data as they arrive.
        let mut stream = client
            .get(SENSOR_API_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?
            .error_for_status() // Ensure the request was successful (status code 2xx)
            .map_err(|e| format!("HTTP error: {}", e))?
            .bytes_stream(); // Convert the response body into a stream of bytes

        // Process each chunk of data from the stream.
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    // Decode the chunk as UTF-8.
                    // Sensor data is often sent as plain text, line by line.
                    let chunk_str = String::from_utf8_lossy(&chunk);

                    // Split the chunk into individual lines, as each line is likely a sensor reading.
                    for line in chunk_str.lines() {
                        // Trim whitespace from the line.
                        let trimmed_line = line.trim();

                        // Skip empty lines.
                        if trimmed_line.is_empty() {
                            continue;
                        }

                        // Parse the line into a SensorReading.
                        // We're expecting a simple format like "timestamp:value"
                        // For a real sensor, you'd have more robust parsing.
                        match parse_sensor_line(trimmed_line) {
                            Ok(reading) => {
                                // We need to send the reading back to the main thread for UI updates.
                                // In a more complex app, you might use a channel.
                                // For this simple case, we'll print it to stdout and rely on the main loop to pick it up.
                                // A better approach for real-world apps would be to use `tokio::sync::mpsc` channels.
                                println!("{}", serde_json::to_string(&reading).unwrap()); // Use JSON for easy parsing in the main loop
                            }
                            Err(e) => {
                                eprintln!("Error parsing line '{}': {}", trimmed_line, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving chunk: {}", e);
                    // In a real app, you might want to retry or handle network errors more gracefully.
                    break; // Stop processing on error
                }
            }
        }
        Ok::<(), Box<dyn Error + Send + Sync>>(()) // Return success to the task
    });

    // This is the main loop that keeps the terminal application running.
    // It handles rendering the UI and processing events (like new data).
    loop {
        // Check if there's new data available from the sensor fetch task.
        // We're using `try_recv` on stdout, which is a bit of a hack for this example.
        // A proper channel (`tokio::sync::mpsc`) would be the idiomatic way to do this.
        // For now, we'll just check if new JSON lines were printed by the fetcher.
        // This part is simplified for clarity and brevity.
        // In a real app, you'd poll a channel.

        // Try to read from stdin, which is where the data fetcher might be printing.
        // This is not a robust way to communicate between tasks.
        // For demonstration purposes, let's assume we can read lines directly.

        // A more robust approach would involve `tokio::sync::mpsc` channel:
        // let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        // data_fetch_handle = tokio::spawn(async move { ... tx.send(reading).await; ... });
        // then in the loop: if let Some(reading) = rx.recv().await { app_state.add_reading(reading); }

        // For this example, we'll simulate receiving data by just checking for lines printed to stdout.
        // This is a significant simplification and not how you'd typically do inter-task communication.
        // In a production environment, use channels.

        // This part is tricky without proper inter-task communication.
        // The `data_fetch_handle` prints JSON, but we don't have a direct way to
        // read it back into this loop without blocking or complex I/O setup.
        // Let's simulate receiving data by having a separate task that *could*
        // push data into `app_state`.

        // For this tutorial, we'll create a *simulated* data stream that gets
        // added directly to app_state *within* this loop for demonstration.
        // This is NOT how you'd handle actual network data, but it allows us to
        // focus on the TUI rendering part.

        // **IMPORTANT:** In a real application, the `data_fetch_handle` would
        // communicate with this main loop using `tokio::sync::mpsc` channels.

        // Simulate receiving a new data point periodically.
        // This will add data directly to app_state without needing to parse stdout.
        // In the context of this tutorial, this replaces the actual network data retrieval.
        // If you had a real API, the `data_fetch_handle` would be sending data to `app_state`.
        // For now, we'll just add a dummy value.
        let current_time = chrono::Utc::now().timestamp() as i64; // Use current time for x-axis simulation
        let simulated_value = (current_time % 100) as f64 + (rand::random::<f64>() * 10.0); // Some varying value

        // Check if the data fetch task has finished.
        if data_fetch_handle.is_finished() {
            // If the data fetch task finished, we should check for errors.
            // The `await` will propagate any errors.
            let _ = data_fetch_handle.await?; // Propagate potential errors from the fetch task
            // Break the loop if the data fetching has ended.
            break;
        }

        // Add the simulated data point to our state.
        app_state.add_reading(SensorReading { timestamp: current_time, value: simulated_value });

        // Render the UI to the terminal.
        terminal.draw(|frame| ui(frame, &app_state))?;

        // Small delay to control the refresh rate and prevent 100% CPU usage.
        // This is important for interactive terminal applications.
        std::thread::sleep(Duration::from_millis(100));

        // **REPLACEMENT FOR REAL DATA HANDLING:**
        // If you were using channels, you'd have something like:
        // if let Ok(reading) = rx.try_recv() { // Use try_recv for non-blocking check
        //     app_state.add_reading(reading);
        // }
        // Then render.

    }

    // Restore the terminal to its normal state.
    // This is crucial for a clean exit.
    crossterm::execute!(stdout(), crossterm::event::DisableMouseCapture)?;
    let _ = _restorer.cancel_paste()?;
    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}

// This function is responsible for drawing the UI elements to the terminal.
fn ui(frame: &mut Frame, app_state: &AppState) {
    // Create a layout for our UI.
    // We divide the terminal into vertical and horizontal chunks.
    let size = frame.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        // Define constraints for each chunk. `Min(1)` means at least 1 row.
        // `Percentage(100)` means take all available space.
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(size);

    // Create a block for our chart.
    let block = Block::default()
        .title("Sensor Data Stream")
        .border_style(Style::default().fg(ratatui::style::Color::Blue)) // Blue border
        .borders(ratatui::widgets::Borders::ALL); // All borders

    // Prepare the data for the chart.
    // `datasets` is a vector of `Dataset`s. We'll have one dataset for our sensor data.
    let datasets = vec![
        Dataset::default()
            .name("Sensor Value") // Name of the dataset
            .data(&app_state.data_points) // The actual data points (x, y)
            .graph_type(GraphType::Line) // Draw as a line graph
            .marker(ratatui::widgets::charts::Marker::Braille) // Braille markers for smoother lines
            .style(Style::default().fg(ratatui::style::Color::Yellow)), // Yellow line color
    ];

    // Configure the X-axis.
    let x_axis = Axis::default()
        .title("Time (Simulated)") // Label for the x-axis
        .labels_alignment(ratatui::layout::Alignment::Center) // Center align labels
        .style(Style::default().fg(ratatui::style::Color::Gray)) // Gray axis color
        // Define visible range for the x-axis.
        // We want to show the last `MAX_DATA_POINTS` worth of x-values.
        .bounds([
            app_state.next_x.saturating_sub(MAX_DATA_POINTS as f64), // Start of the visible range
            app_state.next_x, // End of the visible range
        ]);

    // Configure the Y-axis.
    let y_axis = Axis::default()
        .title("Sensor Reading") // Label for the y-axis
        .labels_alignment(ratatui::layout::Alignment::Right) // Right align labels
        .style(Style::default().fg(ratatui::style::Color::Gray)) // Gray axis color
        // Set a reasonable range for the Y-axis based on expected sensor values.
        // You might need to dynamically adjust this based on incoming data.
        .bounds([0.0, 100.0]); // Example bounds: 0 to 100

    // Create the Chart widget.
    let chart = Chart::new(datasets)
        .block(block) // Attach the block to the chart
        .x_axis(x_axis) // Set the x-axis
        .y_axis(y_axis); // Set the y-axis

    // Render the chart in the center chunk of our layout.
    frame.render_widget(chart, chunks[0]);
}

// A helper function to parse a single line of sensor data.
// Assumes a format like "timestamp:value"
fn parse_sensor_line(line: &str) -> Result<SensorReading, Box<dyn Error>> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() != 2 {
        return Err("Invalid line format. Expected 'timestamp:value'".into());
    }

    let timestamp = parts[0].parse::<i64>()?;
    let value = parts[1].parse::<f64>()?;

    Ok(SensorReading { timestamp, value })
}

// Example Usage:
// To run this example:
// 1. Make sure you have Rust installed.
// 2. Create a new Rust project: `cargo new sensor_visualizer`
// 3. `cd sensor_visualizer`
// 4. Add the necessary dependencies to your `Cargo.toml` file:
//    [dependencies]
//    tokio = { version = "1", features = ["full"] }
//    reqwest = { version = "0.11", features = ["stream"] }
//    ratatui = "0.25"
//    futures = "0.3"
//    crossterm = "0.27"
//    chrono = "0.4"
//    rand = "0.8"
//    serde_json = "1.0"
// 5. Replace the content of `src/main.rs` with the code above.
// 6. Run the application: `cargo run`
//
// You will see a terminal-based graph that updates in real-time with simulated sensor data.
// To test with a real streaming API, you would need to:
// 1. Set up a server that streams sensor data (e.g., using websockets or SSE).
// 2. Replace `SENSOR_API_URL` with the correct URL of your API.
// 3. Ensure the data format from your API can be parsed by `parse_sensor_line`.
//    If your API streams JSON objects directly, you'd use `serde_json::from_str` instead.
//
// For demonstration, this code simulates data if `SENSOR_API_URL` is not reachable or
// if you don't have a live API.