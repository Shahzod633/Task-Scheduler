pub mod db;
pub mod models;
pub mod commands;
pub mod crypto;
pub mod email;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("failed to get app data dir");
            let conn = db::init(&app_dir).expect("failed to initialize db");

            app.manage(db::DbState {
                conn: std::sync::Mutex::new(conn),
                app_dir,
            });

            // Проверка сроков живёт в отдельном потоке, а не в таймере на
            // фронтенде: напоминание должно приходить и тогда, когда окно
            // свёрнуто, а веб-страница в свёрнутом окне засыпает.
            //
            // Первая проверка идёт сразу после запуска — иначе человек,
            // открывший приложение утром и закрывший через десять минут, не
            // узнал бы о сегодняшнем сроке до следующего дня.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                commands::run_deadline_checks(&handle);
                std::thread::sleep(commands::REMINDER_CHECK_INTERVAL);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_workspaces,
            commands::create_workspace,
            commands::update_workspace,
            commands::archive_workspace,
            commands::set_workspace_background,
            commands::clear_workspace_background,
            commands::get_workspace_background,

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

            commands::list_card_comments,
            commands::create_card_comment,
            commands::delete_card_comment,

            commands::get_reminder_settings,
            commands::update_reminder_settings,

            commands::get_email_settings,
            commands::update_email_settings,
            commands::set_email_password,
            commands::clear_email_password,
            commands::send_test_email,

            commands::export_board,
            commands::export_board_to_file,
            commands::import_board,
            commands::import_board_from_file,

            commands::get_archived_columns,
            commands::get_archived_cards,
            commands::restore_card,
            commands::restore_column,
            commands::delete_card,
            commands::delete_column,
            commands::delete_board,

            commands::export_database,
            commands::suggest_export_name,
            commands::get_backups,
            commands::get_backup_dir,
            commands::open_backup_dir,
            commands::get_app_version,

            commands::list_checklist_items,
            commands::create_checklist_item,
            commands::toggle_checklist_item,
            commands::delete_checklist_item,

            commands::list_members,
            commands::create_member,
            commands::update_member,
            commands::delete_member,

            commands::update_card_assignee,
            commands::update_card_author,
            commands::update_card_priority,
            commands::list_all_cards_in_workspace,

            commands::get_labels,
            commands::create_label,
            commands::add_label_to_card,
            commands::remove_label_from_card,

            commands::get_notifications,
            commands::mark_all_notifications_read,

            commands::get_user_profile,
            commands::update_user_profile,
            commands::update_theme,

            commands::record_board_view,
            commands::get_recent_boards,

            commands::get_inbox_column,
            commands::get_cards_with_due_dates,

            commands::mark_card_mistake,
            commands::resolve_card_mistake,
            commands::get_mistake_cards,
            commands::request_card_retry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
