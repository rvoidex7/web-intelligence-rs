#[cfg(feature = "tauri")]
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};
use crate::BrowserLauncherBuilder;

#[cfg(feature = "tauri")]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("web-intelligence")
        .setup(|app, _api| {
            // Retrieve the application's configuration or set defaults.
            // For now, we'll initialize a headless background agent browser by default
            // when the plugin is registered.

            let launcher = BrowserLauncherBuilder::new()
                .profile_name("tauri-web-intel-profile")
                .headless(true)
                .with_ai_flags(true);

            match launcher.launch() {
                Ok(browser_handle) => {
                    tracing::info!(
                        "Web Intelligence Plugin: Browser launched. WebSocket URL: {}",
                        browser_handle.websocket_url()
                    );

                    // Manage the browser handle state within the Tauri application
                    // so it stays alive as long as the app is running.
                    app.manage(browser_handle);
                }
                Err(e) => {
                    tracing::error!("Web Intelligence Plugin: Failed to launch browser: {}", e);
                }
            }
            Ok(())
        })
        .build()
}
