mod commands;
mod runtime;

use commands::{
    add_variable, apply_migration, copy_value, create_group, create_link, delete_variable,
    detach_link_member, list_projects, move_variable, plan_migration, read_value, register_project,
    remove_project, rename_group, save_description, save_value, scan_project, set_codex_access,
};
use runtime::AppRuntime;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let runtime = AppRuntime::load(app.handle())?;
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            register_project,
            remove_project,
            scan_project,
            save_value,
            save_description,
            create_group,
            rename_group,
            add_variable,
            delete_variable,
            move_variable,
            create_link,
            detach_link_member,
            set_codex_access,
            read_value,
            copy_value,
            plan_migration,
            apply_migration,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Env Manager");
}
