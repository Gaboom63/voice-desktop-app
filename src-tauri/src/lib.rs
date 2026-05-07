use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_notification::NotificationExt;

// Helper function to extract our embedded icon to disk so notify-send can use it
fn get_default_icon_path() -> String {
    let path = std::env::temp_dir().join("gv_default_notify_icon.png");
    // Write the embedded icon to /tmp/ silently
    let _ = std::fs::write(&path, include_bytes!("../icons/Google-Voice-Notifcation-Icon.png"));
    path.to_string_lossy().into_owned()
}

#[tauri::command]
fn update_status(app: tauri::AppHandle, count: i32, notify: bool) {
    if let Some(tray) = app.tray_by_id("main_tray") {
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
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("notify-send")
            .arg("-a")
            .arg("Google Voice")
            .arg("-i")
            .arg(get_default_icon_path()) 
            .arg("Google Voice")
            .arg(format!("You have {} unread message(s)", count))
            .spawn();
    }
}

#[tauri::command]
async fn trigger_rich_notification(
    app: tauri::AppHandle, 
    title: String, 
    body: String, 
    avatar_url: Option<String>
) -> Result<(), String> {
    // 1. Update the tray icon
    if let Some(tray) = app.tray_by_id("main_tray") {
        let default_icon_bytes = include_bytes!("../icons/Google-Voice-Notifcation-Icon.png").as_slice();
        if let Ok(img) = image::load_from_memory(default_icon_bytes) {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
            let _ = tray.set_icon(Some(icon));
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 2. Default to our custom extracted Google Voice icon
        let mut icon_path = get_default_icon_path(); 

        // 3. If JS gave us a URL, download it natively using reqwest (Bypassing CORS)
        if let Some(url) = avatar_url {
            if let Ok(response) = reqwest::get(&url).await {
                if let Ok(bytes) = response.bytes().await {
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                        
                    let temp_path = std::env::temp_dir().join(format!("gv_avatar_{}.png", timestamp));
                    
                    if std::fs::write(&temp_path, bytes).is_ok() {
                        icon_path = temp_path.to_string_lossy().into_owned();
                    }
                }
            }
        }

        // 4. Fire the notification
        let _ = std::process::Command::new("notify-send")
            .arg("-a")
            .arg("Google Voice")
            .arg("-i")
            .arg(icon_path)
            .arg(&title)
            .arg(&body)
            .spawn();
    }
    
    Ok(())
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

                if ('serviceWorker' in navigator) {
                    try {
                        Object.defineProperty(navigator, 'serviceWorker', { 
                            value: undefined, 
                            configurable: true 
                        });
                    } catch(e) {}
                }

                let lastCount = 0;
                
                setInterval(async () => {
                    let match = document.title.match(/\((\d+)\)/);
                    let count = match ? parseInt(match[1]) : 0;
                    
                    if (count > lastCount) {
                        let senderName = "Google Voice";
                        let messageText = "You have a new message.";
                        let avatarUrl = null;

                        try {
                            let threads = Array.from(document.querySelectorAll('.thread-details'));
                            let unreadThread = threads.find(t => t.textContent.includes('Unread'));
                            
                            if (unreadThread) {
                                let nameEl = unreadThread.querySelector('.participants');
                                let previewEl = unreadThread.querySelector('.preview');
                                
                                if (nameEl) senderName = nameEl.textContent.trim();
                                if (previewEl) messageText = previewEl.textContent.trim();

                                let parentRow = unreadThread.parentElement;
                                if (parentRow) {
                                    let imgEl = parentRow.querySelector('img');
                                    if (imgEl && imgEl.src && !imgEl.src.includes('data:')) {
                                        // FORCE HTTPS to fix Mixed Content blocking
                                        avatarUrl = imgEl.src.replace('http://', 'https://');
                                    }
                                }
                            }
                        } catch (e) {
                            console.error("DOM Scrape Error: ", e);
                        }

                        // Send the URL directly to Rust instead of fetching bytes
                        window.__TAURI_INTERNALS__.invoke("trigger_rich_notification", {
                            title: senderName,
                            body: messageText,
                            avatarUrl: avatarUrl
                        });

                        window.__TAURI_INTERNALS__.invoke("update_status", { 
                            count: count,
                            notify: false 
                        });
                        
                    } else if (count !== lastCount) {
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