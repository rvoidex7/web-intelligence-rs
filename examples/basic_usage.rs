use web_intelligence::{BrowserLauncherBuilder, WebIntelError, AIExecutionStrategy};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), WebIntelError> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt::init();

    println!("Launching browser...");

    // Create a new browser launcher
    // We use ephemeral mode for this example so it doesn't leave data behind
    let mut handle = BrowserLauncherBuilder::new()
        .ephemeral(true)
        .headless(false) // Changed to false to see the app mode window (if we weren't in a headless environment)
        .with_ai_flags(true)
        .viewport(1280, 720)
        // New features:
        .app_mode(true)
        .start_url("https://google.com") // In a real app, this would be your local frontend URL
        .with_ai_strategy(AIExecutionStrategy::Hybrid)
        .openai_api_key("sk-proj-...") // Placeholder key
        .launch()?;

    println!("Browser launched successfully!");
    println!("WebSocket URL: {}", handle.websocket_url());

    // In a real application, you would connect to the WebSocket URL here
    // using a crate like `tungstenite` or `chrome_remote_interface`.

    println!("Browser is running. Waiting for 5 seconds...");
    thread::sleep(Duration::from_secs(5));

    println!("Shutting down...");
    // When `handle` goes out of scope, the browser process is killed
    // and the temporary profile directory is deleted.

    Ok(())
}
