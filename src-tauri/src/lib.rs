mod commands;
mod integrations;
mod runtime;

use commands::{
    add_variable, apply_gitignore_guard, apply_migration, copy_key, copy_value, create_group,
    create_link, delete_variable, detach_link_member, export_env_files, install_agent_integration,
    list_agent_activity, list_agent_integrations, list_projects, move_variable, plan_migration,
    protect_variables, read_value, register_project, remove_project, rename_env_file, rename_group,
    rename_project, save_description, save_value, scan_project, set_codex_access,
};
use runtime::AppRuntime;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            let runtime = AppRuntime::load(app.handle())?;
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            list_agent_integrations,
            install_agent_integration,
            register_project,
            remove_project,
            rename_project,
            rename_env_file,
            export_env_files,
            scan_project,
            apply_gitignore_guard,
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
            protect_variables,
            list_agent_activity,
            read_value,
            copy_key,
            copy_value,
            plan_migration,
            apply_migration,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Env Manager");
}
