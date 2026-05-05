use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

// --- NEW: Custom command to swap the tray icon using RGBA decoding ---
#[tauri::command]
fn set_tray_status(app: tauri::AppHandle, has_unread: bool) {
    if let Some(tray) = app.tray_by_id("main_tray") {
        let icon_bytes = if has_unread {
            include_bytes!("../icons/Google-Voice-Notifcation-Icon.png").as_slice()
        } else {
            include_bytes!("../icons/Google-Voice-Normal-Icon.png").as_slice()
        };

        // Decode the PNG into raw RGBA pixels
        if let Ok(img) = image::load_from_memory(icon_bytes) {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            // Pass the raw pixels to Tauri v2's expected format
            let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
            let _ = tray.set_icon(Some(icon));
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![set_tray_status])
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

                let lastCount = 0;
                setInterval(() => {
                    let match = document.title.match(/\((\d+)\)/);
                    let count = match ? parseInt(match[1]) : 0;
                    
                    if (count !== lastCount) {
                        window.__TAURI_INTERNALS__.invoke("set_tray_status", { 
                            hasUnread: count > 0 
                        });
                    }

                    if (count > lastCount) {
                        window.__TAURI_INTERNALS__.invoke("plugin:notification|notify", {
                            options: {
                                title: "Google Voice",
                                body: `You have ${count} unread message(s)`
                            }
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
            
            // Set the default normal icon on startup by decoding the PNG
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