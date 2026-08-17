mod commands;
mod integrations;
mod runtime;

use commands::{
    add_variable, apply_gitignore_guard, apply_migration, apply_team_import,
    compare_provider_values, connect_folder_team_channel, copy_key, copy_value,
    create_github_environment, create_group, create_link, delete_variable, detach_link_member,
    detect_cloudflare_target, detect_github_repository, discard_team_import, export_env_files,
    get_last_selected_project_id, inspect_aws_access, inspect_cloudflare_access,
    install_agent_integration, install_personal_provider_pack, list_agent_activity,
    list_agent_integrations, list_deployment_providers, list_github_environments,
    list_github_repositories, list_projects, list_provider_push_receipts, list_runtime_targets,
    list_team_channels, move_variable, plan_migration, plan_team_channel_import, plan_team_import,
    protect_variables, publish_team_channel, push_to_provider, read_value, register_project,
    remap_team_import_file, remove_personal_provider_pack, remove_project, remove_runtime_target,
    remove_team_channel, rename_env_file, rename_group, rename_project,
    reveal_team_import_conflict, save_description, save_runtime_target, save_value, scan_project,
    set_codex_access, set_last_selected_project,
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
            get_last_selected_project_id,
            set_last_selected_project,
            list_agent_integrations,
            install_agent_integration,
            install_personal_provider_pack,
            remove_personal_provider_pack,
            list_deployment_providers,
            list_github_repositories,
            detect_github_repository,
            detect_cloudflare_target,
            inspect_cloudflare_access,
            inspect_aws_access,
            list_github_environments,
            create_github_environment,
            push_to_provider,
            compare_provider_values,
            list_runtime_targets,
            save_runtime_target,
            remove_runtime_target,
            list_provider_push_receipts,
            register_project,
            remove_project,
            rename_project,
            rename_env_file,
            export_env_files,
            list_team_channels,
            connect_folder_team_channel,
            remove_team_channel,
            publish_team_channel,
            plan_team_channel_import,
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
