#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Backend requests run through this plugin rather than the webview:
        // the app is served from `http://tauri.localhost`, which makes every
        // call to the API cross-site and drops the session cookie.
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
