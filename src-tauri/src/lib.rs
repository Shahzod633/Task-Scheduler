pub mod db;
pub mod models;
pub mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("failed to get app data dir");
            let conn = db::init(&app_dir).expect("failed to initialize db");
            
            app.manage(db::DbState {
                conn: std::sync::Mutex::new(conn),
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_workspaces,
            commands::create_workspace,
            commands::update_workspace,
            commands::archive_workspace,
            
            commands::get_boards,
            commands::get_board,
            commands::create_board,
            commands::update_board,
            commands::archive_board,
            commands::get_archived_boards,
            commands::restore_board,
            
            commands::get_columns,
            commands::create_column,
            commands::update_column,
            commands::archive_column,
            commands::reorder_columns,

            commands::get_cards,
            commands::create_card,
            commands::update_card,
            commands::archive_card,
            commands::update_card_position,
            commands::export_board,

            commands::get_labels,
            commands::create_label,
            commands::add_label_to_card,
            commands::remove_label_from_card,

            commands::get_notifications,
            commands::mark_all_notifications_read,

            commands::get_user_profile,
            commands::update_user_profile,

            commands::record_board_view,
            commands::get_recent_boards,

            commands::get_inbox_column,
            commands::get_cards_with_due_dates,

            commands::mark_card_mistake,
            commands::resolve_card_mistake,
            commands::get_mistake_cards,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
