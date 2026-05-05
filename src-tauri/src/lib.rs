use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_notification::NotificationExt;

// --- Command 1: The Background Timer (Generic "Unread Messages" check) ---
#[tauri::command]
fn update_status(app: tauri::AppHandle, count: i32, notify: bool) {
    if let Some(tray) = app.tray_by_id("main_tray") {
        // BACK TO NORMAL: Swaps between Red Dot and Normal based on count
        let icon_bytes = if count > 0 {
            include_bytes!("../icons/Google-Voice-Notifcation-Icon.png").as_slice()
        } else {
            include_bytes!("../icons/Google-Voice-Normal-Icon.png").as_slice()
        };

        if let Ok(img) = image::load_from_memory(icon_bytes) {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
            let _ = tray.set_icon(Some(icon));
        }
    }

    if notify {
        let _ = app.notification()
            .builder()
            .title("Google Voice")
            .body(format!("You have {} unread message(s)", count))
            .show();
    }
}

// --- Command 2: The Rich Notification (Contact Name + Exact Message) ---
#[tauri::command]
fn trigger_rich_notification(app: tauri::AppHandle, title: String, body: String) {
    // Force the red dot ON when a rich notification arrives
    if let Some(tray) = app.tray_by_id("main_tray") {
        let icon_bytes = include_bytes!("../icons/Google-Voice-Notifcation-Icon.png").as_slice();
        if let Ok(img) = image::load_from_memory(icon_bytes) {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
            let _ = tray.set_icon(Some(icon));
        }
    }

    // Fire the native Linux notification with the specific name and text
    let _ = app.notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![update_status, trigger_rich_notification])
        .setup(|app| {
            let window = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(tauri::Url::parse("https://voice.google.com").unwrap())
            )
            .title("Google Voice")
            .inner_size(1100.0, 750.0)
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .initialization_script(r#"
                window.open = function(url, name, specs) {
                    window.location.href = url;
                    return window; 
                };
                
                window.addEventListener('DOMContentLoaded', () => {
                    document.body.addEventListener('click', (e) => {
                        let a = e.target.closest('a');
                        if (a && a.target === '_blank') {
                            e.preventDefault();
                            window.location.href = a.href;
                        }
                    });
                });

                const mockNotification = function(title, options) {
                    window.__TAURI_INTERNALS__.invoke("trigger_rich_notification", {
                        title: title || "New Message",
                        body: options ? (options.body || "") : ""
                    });
                    
                    return {
                        onclick: null,
                        close: function() {},
                        addEventListener: function() {},
                        removeEventListener: function() {},
                        dispatchEvent: function() { return true; }
                    };
                };
                mockNotification.permission = "granted";
                mockNotification.requestPermission = async function() { return "granted"; };

                try { 
                    window.Notification = mockNotification; 
                } catch (e) {
                    Object.defineProperty(window, 'Notification', {
                        value: mockNotification, 
                        writable: true, 
                        configurable: true
                    });
                }

                let lastCount = 0;
                setInterval(() => {
                    let match = document.title.match(/\((\d+)\)/);
                    let count = match ? parseInt(match[1]) : 0;
                    
                    if (count !== lastCount) {
                        window.__TAURI_INTERNALS__.invoke("update_status", { 
                            count: count,
                            notify: false 
                        });
                    }
                    lastCount = count;
                }, 2000);
            "#)
            .build()?;

            let window_clone = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
                _ => {}
            });

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show Google Voice", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let mut tray_builder = TrayIconBuilder::with_id("main_tray").menu(&menu);
            
            // Startup with the Normal tray icon
            let default_icon_bytes = include_bytes!("../icons/Google-Voice-Normal-Icon.png").as_slice();
            if let Ok(img) = image::load_from_memory(default_icon_bytes) {
                let rgba = img.into_rgba8();
                let (width, height) = rgba.dimensions();
                let default_image = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
                tray_builder = tray_builder.icon(default_image);
            }

            let _tray = tray_builder
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}