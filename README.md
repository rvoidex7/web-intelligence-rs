# web-intelligence

A Rust library that launches Chromium-based browsers (Chrome, Edge) with "Built-in AI" features (Gemini Nano, `window.ai`) enabled. It creates isolated profiles for development, keeping your main browser profile clean.

This crate is especially useful for **Tauri, Electron, and Wails** developers who want to test and integrate local browser AI capabilities into their desktop web applications without bundling heavy LLMs.

## Features

- **Automatic Browser Detection:** Finds Chrome, Canary, Chromium, or Edge installations across Windows, macOS, and Linux.
- **AI-Ready Flags:** Automatically configures the complex CLI flags required to enable on-device AI (`PromptAPIForGeminiNano`, etc.).
- **Tauri Drop-in Plugin:** Easily integrate a headless AI browser agent into your Tauri application.
- **Frontend TypeScript SDK:** Provides a robust `AIClient` wrapper for `window.ai` that supports both new and legacy APIs, WebMCP tool calling, and falls back to Cloud APIs (like OpenAI) if local hardware is insufficient.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
web-intelligence = "0.1.0"
```

### Tauri Integration

If you are building a Tauri application, you can use the built-in plugin to spawn a headless AI browser in the background.

```toml
[dependencies]
web-intelligence = { version = "0.1.0", features = ["tauri"] }
```

In your `src-tauri/src/main.rs`:

```rust
fn main() {
    tauri::Builder::default()
        // Initializes a headless background browser with AI flags
        .plugin(web_intelligence::tauri_plugin::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Standalone Rust Usage

```rust
use web_intelligence::BrowserLauncherBuilder;

fn main() {
    let launcher = BrowserLauncherBuilder::new()
        .profile_name("my-ai-dev-profile")
        .headless(false)
        .with_ai_flags(true);

    match launcher.launch() {
        Ok(child) => println!("Browser launched. WebSocket URL: {}", child.websocket_url()),
        Err(e) => eprintln!("Error launching browser: {}", e),
    }
}
```

## Frontend SDK Usage

Copy `frontend/ai-sdk.ts` to your web project. The SDK simplifies interactions with the browser's native AI.

```typescript
import { AIClient } from './ai-sdk';

const ai = new AIClient({ strategy: 'hybrid', openaiKey: 'sk-...' });

async function run() {
  await ai.init(); // Auto-detects local hardware capabilities, falls back to cloud if needed

  const stream = ai.stream("Explain quantum computing simply.");
  for await (const chunk of stream) {
    process.stdout.write(chunk);
  }
}
```

## Related Resources
- [Chrome Built-in AI Documentation](https://developer.chrome.com/docs/ai/built-in)
- [Model Context Protocol (WebMCP)](https://modelcontextprotocol.io/)