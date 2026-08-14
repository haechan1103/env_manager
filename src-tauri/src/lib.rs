mod commands;
mod integrations;
mod provider_push;
mod runtime;

use commands::{
    add_variable, apply_gitignore_guard, apply_migration, apply_team_import, copy_key, copy_value,
    create_github_environment, create_group, create_link, delete_variable, detach_link_member,
    detect_cloudflare_target, detect_github_repository, discard_team_import, export_env_files,
    install_agent_integration, list_agent_activity, list_agent_integrations,
    list_deployment_providers, list_github_environments, list_github_repositories, list_projects,
    move_variable, plan_migration, plan_team_import, protect_variables, push_to_provider,
    read_value, register_project, remap_team_import_file, remove_project, rename_env_file,
    rename_group, rename_project, reveal_team_import_conflict, save_description, save_value,
    scan_project, set_codex_access,
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
            list_deployment_providers,
            list_github_repositories,
            detect_github_repository,
            detect_cloudflare_target,
            list_github_environments,
            create_github_environment,
            push_to_provider,
            register_project,
            remove_project,
            rename_project,
            rename_env_file,
            export_env_files,
            plan_team_import,
            remap_team_import_file,
            reveal_team_import_conflict,
            apply_team_import,
            discard_team_import,
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
