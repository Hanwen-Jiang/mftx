mod commands;
mod compat;
mod events;
mod models;
mod runtime;

use commands::{
    complete_setup, connect_peer, discover_peers, get_app_state, get_default_setup, list_inbox,
    list_trusted_devices, open_inbox, pull_from_peer, respond_incoming_transfer, reveal_path,
    send_paths, show_main_window, start_peer, stop_peer, untrust_device, update_settings,
};
use runtime::DesktopRuntime;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(DesktopRuntime::default())
        .setup(|app| {
            create_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_default_setup,
            complete_setup,
            update_settings,
            start_peer,
            stop_peer,
            list_trusted_devices,
            untrust_device,
            respond_incoming_transfer,
            discover_peers,
            connect_peer,
            list_inbox,
            send_paths,
            pull_from_peer,
            open_inbox,
            reveal_path,
            show_main_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MFTX desktop");
}

fn create_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open MFTX", true, None::<&str>)?;
    let inbox = MenuItem::with_id(app, "inbox", "Open Inbox", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &inbox, &quit])?;

    TrayIconBuilder::with_id("mftx")
        .tooltip("MFTX Desktop")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "inbox" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(runtime) = handle.try_state::<DesktopRuntime>() {
                        let _ = crate::commands::open_inbox(runtime).await;
                    }
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
