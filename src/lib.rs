use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use regex::Regex;
use tempfile::TempDir;
use tracing::{debug, info};
use which::which;

mod error;
pub use error::WebIntelError;

/// Strategies for AI Execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIExecutionStrategy {
    /// Forces usage of the built-in Window AI. Fails if unsupported.
    LocalOnly,
    /// Skips local checks, always uses cloud API key.
    CloudOnly,
    /// Tries Local first; if hardware/browser is insufficient, falls back to Cloud.
    Hybrid,
}

impl Default for AIExecutionStrategy {
    fn default() -> Self {
        Self::LocalOnly
    }
}

/// Represents the viewport size for the browser window.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// A builder to configure and launch a Chromium-based browser with AI capabilities.
#[derive(Debug)]
pub struct BrowserLauncherBuilder {
    profile_name: String,
    viewport: Option<Viewport>,
    headless: bool,
    extensions: Vec<PathBuf>,
    browser_executable: Option<PathBuf>,
    ephemeral: bool,
    with_ai_flags: bool,
    extra_args: Vec<String>,
    app_mode: bool,
    start_url: Option<String>,
    strategy: AIExecutionStrategy,
    openai_api_key: Option<String>,
    anthropic_api_key: Option<String>,
}

impl Default for BrowserLauncherBuilder {
    fn default() -> Self {
        Self {
            profile_name: "web-intel-profile".to_string(),
            viewport: None,
            headless: false,
            extensions: Vec::new(),
            browser_executable: None,
            ephemeral: false,
            with_ai_flags: true,
            extra_args: Vec::new(),
            app_mode: false,
            start_url: None,
            strategy: AIExecutionStrategy::default(),
            openai_api_key: None,
            anthropic_api_key: None,
        }
    }
}

impl BrowserLauncherBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the profile name. Used for directory naming in persistent mode.
    pub fn profile_name(mut self, name: impl Into<String>) -> Self {
        self.profile_name = name.into();
        self
    }

    /// Sets the viewport size.
    pub fn viewport(mut self, width: u32, height: u32) -> Self {
        self.viewport = Some(Viewport { width, height });
        self
    }

    /// Enable or disable headless mode.
    /// Crucial for background agents.
    pub fn headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    /// Add an extension to load.
    pub fn with_extension(mut self, path: impl AsRef<Path>) -> Self {
        self.extensions.push(path.as_ref().to_path_buf());
        self
    }

    /// Set a custom browser executable path.
    pub fn browser_executable(mut self, path: impl AsRef<Path>) -> Self {
        self.browser_executable = Some(path.as_ref().to_path_buf());
        self
    }

    /// If true, uses a temporary directory for the profile which is deleted when the handle is dropped.
    pub fn ephemeral(mut self, ephemeral: bool) -> Self {
        self.ephemeral = ephemeral;
        self
    }

    /// Enable or disable the default "Built-in AI" flags (Gemini Nano, etc.).
    /// Defaults to true.
    pub fn with_ai_flags(mut self, enabled: bool) -> Self {
        self.with_ai_flags = enabled;
        self
    }

    /// Add extra arguments to the browser command.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Enable application mode.
    /// If true, launches the browser with --app=<URL> (frameless window).
    pub fn app_mode(mut self, enabled: bool) -> Self {
        self.app_mode = enabled;
        self
    }

    /// Set the starting URL.
    pub fn start_url(mut self, url: impl Into<String>) -> Self {
        self.start_url = Some(url.into());
        self
    }

    /// Set the AI execution strategy.
    pub fn with_ai_strategy(mut self, strategy: AIExecutionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the OpenAI API key.
    pub fn openai_api_key(mut self, key: impl Into<String>) -> Self {
        self.openai_api_key = Some(key.into());
        self
    }

    /// Set the Anthropic API key.
    pub fn anthropic_api_key(mut self, key: impl Into<String>) -> Self {
        self.anthropic_api_key = Some(key.into());
        self
    }

    /// Launches the browser with the configured settings.
    pub fn launch(self) -> Result<BrowserHandle, WebIntelError> {
        let browser_path = self.find_browser_executable()?;
        debug!("Using browser executable: {:?}", browser_path);

        let user_data_dir = if self.ephemeral {
            UserDataDir::Ephemeral(
                tempfile::Builder::new()
                    .prefix("web-intel-")
                    .tempdir()
                    .map_err(WebIntelError::ProfileCreationFailure)?
            )
        } else {
            let mut path = dirs::cache_dir().ok_or_else(|| {
                WebIntelError::ProfileCreationFailure(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not determine cache directory",
                ))
            })?;
            path.push(&self.profile_name);
            std::fs::create_dir_all(&path).map_err(WebIntelError::ProfileCreationFailure)?;
            UserDataDir::Persistent(path)
        };
        debug!("Using user data directory: {:?}", user_data_dir.path());

        let mut cmd = Command::new(browser_path);

        // Basic flags
        cmd.arg(format!("--user-data-dir={}", user_data_dir.path().display()));
        cmd.arg("--remote-debugging-port=0"); // Let the OS pick a free port
        cmd.arg("--no-first-run");
        cmd.arg("--no-default-browser-check");

        if self.headless {
            cmd.arg("--headless=new");
        }

        if let Some(viewport) = self.viewport {
            cmd.arg(format!("--window-size={},{}", viewport.width, viewport.height));
        }

        if !self.extensions.is_empty() {
            let paths: Vec<String> = self.extensions.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            cmd.arg(format!("--load-extension={}", paths.join(",")));
        }

        if self.with_ai_flags {
            // Flags for enabling experimental AI features
            cmd.arg("--enable-features=OptimizationGuideModelDownloading,OptimizationGuideOnDeviceModel,PromptAPIForGeminiNano");
            cmd.arg("--allow-insecure-localhost");
        }

        // Application Mode Logic
        if let Some(url) = &self.start_url {
            if self.app_mode {
                cmd.arg(format!("--app={}", url));
            } else {
                cmd.arg(url);
            }
        }

        for arg in self.extra_args {
            cmd.arg(arg);
        }

        // Inject configuration as Environment Variables
        if let Some(key) = &self.openai_api_key {
            cmd.env("OPENAI_API_KEY", key);
        }
        if let Some(key) = &self.anthropic_api_key {
            cmd.env("ANTHROPIC_API_KEY", key);
        }

        let strategy_str = match self.strategy {
            AIExecutionStrategy::LocalOnly => "local",
            AIExecutionStrategy::CloudOnly => "cloud",
            AIExecutionStrategy::Hybrid => "hybrid",
        };
        cmd.env("WEB_INTEL_STRATEGY", strategy_str);

        // Capture stderr to find the DevTools WebSocket URL
        cmd.stderr(Stdio::piped());
        // We don't need stdout, so we discard it to avoid filling the pipe and deadlocking
        cmd.stdout(Stdio::null());

        let mut child = cmd.spawn().map_err(WebIntelError::LaunchFailure)?;

        // Need to read stderr to find the WebSocket URL.
        // We do this in a non-blocking way or spawn a thread?
        // Spawning a thread to read until we find the URL or timeout seems appropriate.
        // Since we need to return the handle, but also the URL, we might need to wait a bit.

        let stderr = child.stderr.take().ok_or(WebIntelError::OutputReadFailure)?;
        let websocket_url = Arc::new(Mutex::new(None));
        let ws_clone = websocket_url.clone();

        // Spawn a thread to read stderr and extract the WS URL
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let re = Regex::new(r"ws://127\.0\.0\.1:\d+/devtools/browser/[\w-]+").expect("Invalid Regex");

            for line in reader.lines() {
                if let Ok(l) = line {
                    // Log output for debugging
                    debug!("[Browser]: {}", l);
                    if let Some(caps) = re.find(&l) {
                        let mut guard = ws_clone.lock().unwrap();
                        *guard = Some(caps.as_str().to_string());
                        // Once found, we can continue reading or just let it be.
                        // Often we want to keep draining the pipe to avoid blocking.
                    }
                }
            }
        });

        // Wait a short duration for the WS URL to appear
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(10);
        let mut found_url = None;

        while start.elapsed() < timeout {
            {
                let guard = websocket_url.lock().unwrap();
                if let Some(ref url) = *guard {
                    found_url = Some(url.clone());
                    break;
                }
            }
            if let Ok(Some(_status)) = child.try_wait() {
                // Process exited early
                return Err(WebIntelError::LaunchFailure(std::io::Error::new(std::io::ErrorKind::Other, "Browser process exited unexpectedly")));
            }
            thread::sleep(Duration::from_millis(100));
        }

        let url = found_url.ok_or(WebIntelError::WebSocketUrlNotFound)?;
        info!("Browser launched. WebSocket URL: {}", url);

        Ok(BrowserHandle {
            process: child,
            websocket_url: url,
            _user_data_dir: user_data_dir,
        })
    }

    fn find_browser_executable(&self) -> Result<PathBuf, WebIntelError> {
        if let Some(ref path) = self.browser_executable {
            if path.exists() {
                return Ok(path.clone());
            }
            return Err(WebIntelError::BrowserNotFound);
        }

        let mut candidates = Vec::new();

        if cfg!(target_os = "windows") {
             // Standard PATH binaries
            candidates.extend(vec!["chrome.exe".to_string(), "msedge.exe".to_string(), "chromium.exe".to_string()]);
            
            // Common Windows Installation Paths
            let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string());
            let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".to_string());
            let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| r"C:\Users\Default\AppData\Local".to_string());

            candidates.push(format!(r"{}\Google\Chrome SxS\Application\chrome.exe", local_app_data)); // Canary
            candidates.push(format!(r"{}\Google\Chrome\Application\chrome.exe", program_files));
            candidates.push(format!(r"{}\Google\Chrome\Application\chrome.exe", program_files_x86));
            candidates.push(format!(r"{}\Microsoft\Edge\Application\msedge.exe", program_files));
            candidates.push(format!(r"{}\Microsoft\Edge\Application\msedge.exe", program_files_x86));

        } else if cfg!(target_os = "macos") {
            candidates.extend(vec![
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary".to_string(),
                "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev".to_string(),
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string(),
                "/Applications/Chromium.app/Contents/MacOS/Chromium".to_string(),
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".to_string()
            ]);
        } else {
             // Linux
            candidates.extend(vec![
                "google-chrome-unstable".to_string(),
                "google-chrome-beta".to_string(),
                "google-chrome".to_string(),
                "google-chrome-stable".to_string(),
                "chromium".to_string(),
                "chromium-browser".to_string()
            ]);
        };

        for candidate in candidates {
            let path = PathBuf::from(&candidate);
            if path.is_absolute() {
                if path.exists() {
                    return Ok(path);
                }
            } else {
                if let Ok(p) = which(&candidate) {
                    return Ok(p);
                }
            }
        }

        Err(WebIntelError::BrowserNotFound)
    }
}

/// A wrapper around the profile directory to handle ephemeral vs persistent storage.
#[derive(Debug)]
enum UserDataDir {
    Persistent(PathBuf),
    Ephemeral(TempDir),
}

impl UserDataDir {
    fn path(&self) -> &Path {
        match self {
            UserDataDir::Persistent(p) => p,
            UserDataDir::Ephemeral(t) => t.path(),
        }
    }
}

/// A handle to the running browser process.
///
/// When this struct is dropped, the ephemeral user data directory (if used) is cleaned up,
/// but the browser process itself is NOT automatically killed unless you explicitly do so,
/// though idiomatic Rust wrappers often kill child processes on drop.
///
/// Note: The standard `std::process::Child` does NOT kill on drop.
/// However, for an "Agent" workflow, it might be desirable to kill the browser when the handle is dropped.
/// Let's implement kill on drop for safety, to prevent zombie browser processes.
pub struct BrowserHandle {
    process: Child,
    websocket_url: String,
    // Kept alive to prevent deletion until Drop
    _user_data_dir: UserDataDir,
}

impl BrowserHandle {
    /// Returns the DevTools WebSocket URL.
    pub fn websocket_url(&self) -> &str {
        &self.websocket_url
    }

    /// Access the underlying Child process.
    pub fn process(&mut self) -> &mut Child {
        &mut self.process
    }
}

impl Drop for BrowserHandle {
    fn drop(&mut self) {
        // We attempt to kill the browser process when the handle is dropped.
        // This ensures that we don't leave stray browser instances running
        // after the agent finishes or crashes.
        let _ = self.process.kill();
        let _ = self.process.wait();
        debug!("Browser process terminated.");
    }
}
