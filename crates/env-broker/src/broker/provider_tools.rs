use super::super::*;
use super::project_tools::load_registry_data;

impl Broker {
    pub(super) fn list_deployment_providers(
        &self,
        args: ListProvidersArgs,
    ) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let app_data = self.provider_app_data()?;
        let providers = env_provider::provider_push::list(service.root(), &app_data);
        self.audit(
            service.project_id(),
            "list_deployment_providers",
            &[],
            &[],
            "redacted-provider-metadata",
            "OK",
        );
        serde_json::to_value(providers).map_err(EnvError::serialization)
    }

    pub(super) fn list_action_packs(&self, args: ListProvidersArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let app_data = self.provider_app_data()?;
        let packs = env_provider::action_pack::list(service.root(), &app_data);
        self.audit(
            service.project_id(),
            "list_action_packs",
            &[],
            &[],
            "redacted-action-metadata",
            "OK",
        );
        serde_json::to_value(packs).map_err(EnvError::serialization)
    }

    pub(super) fn list_runtime_targets(&self, args: ListProvidersArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let targets = env_provider::runtime_target::list(service.root())
            .map_err(provider_error)?
            .into_iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "displayName": target.display_name,
                    "sourceFile": target.source_file,
                    "transport": target.transport_label(),
                })
            })
            .collect::<Vec<_>>();
        self.audit(
            service.project_id(),
            "list_runtime_targets",
            &[],
            &[],
            "redacted-runtime-target-metadata",
            "OK",
        );
        Ok(Value::Array(targets))
    }

    pub(super) fn list_team_channels(&self, args: ListTeamChannelsArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let registry = load_registry_data(&self.registry_path()?)?;
        let channels = registry
            .team_channels
            .into_iter()
            .filter(|channel| channel.project_id == service.project_id())
            .map(|channel| {
                let transport = match env_team::open_transport(&channel.transport) {
                    Ok(transport) => transport,
                    Err(error) if error.code() == EnvErrorCode::Io => {
                        return Ok(BrokerTeamChannelProjection {
                            id: channel.id,
                            name: channel.name,
                            readable: false,
                            publishable: None,
                            packages: Vec::new(),
                            requires_human_passphrase: true,
                        });
                    }
                    Err(error) => return Err(error),
                };
                let capabilities = transport.inspect(env_team::CapabilityProbe::ReadOnly)?;
                let packages = if capabilities.readable {
                    transport.list_packages()?
                } else {
                    Vec::new()
                };
                Ok(BrokerTeamChannelProjection {
                    id: channel.id,
                    name: channel.name,
                    readable: capabilities.readable,
                    publishable: capabilities.publishable,
                    packages,
                    requires_human_passphrase: true,
                })
            })
            .collect::<Result<Vec<_>, EnvError>>()?;
        self.audit(
            service.project_id(),
            "list_team_channels",
            &[],
            &[],
            "redacted-channel-metadata",
            "OK",
        );
        serde_json::to_value(channels).map_err(EnvError::serialization)
    }

    pub(super) fn plan_provider_push(&self, args: PlanProviderPushArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        if args.selections.is_empty() || args.selections.len() > 100 {
            return Err(EnvError::invalid("전송할 변수를 1개 이상 선택해주세요."));
        }
        let keys = args
            .selections
            .iter()
            .map(|selection| selection.key.clone())
            .collect::<Vec<_>>();
        let unique = keys.iter().collect::<std::collections::BTreeSet<_>>();
        if unique.len() != keys.len() {
            return Err(EnvError::invalid("같은 변수를 중복 선택할 수 없습니다."));
        }
        let destination = if args.provider == "expo-eas" {
            match args.eas_project.as_deref() {
                Some(project) => format!("{project} [{}]", args.eas_environments.join(", ")),
                None => "대상 미지정".to_owned(),
            }
        } else {
            args.provider.clone()
        };
        let request = ProviderPushRequest {
            provider: args.provider.clone(),
            file: args.file.clone(),
            selections: args.selections,
            repository: args.repository,
            github_environment: args.github_environment,
            worker: args.worker,
            cloudflare_environment: args.cloudflare_environment,
            eas_project: args.eas_project,
            eas_environments: args.eas_environments,
            personal_target: args.personal_target,
            aws_profile: args.aws_profile,
            aws_region: args.aws_region,
            aws_path_prefix: args.aws_path_prefix,
            aws_kms_key_id: args.aws_kms_key_id,
        };
        self.store_plan(
            &service,
            PlannedOperation::ProviderPush(request),
            format!(
                "{}의 환경변수 {}개를 {} 대상으로 값 노출 없이 전송합니다.",
                args.file,
                keys.len(),
                destination
            ),
            vec![args.file],
            keys,
            "opaque-provider-push",
            None,
        )
    }

    pub(super) fn plan_action(&self, args: PlanActionArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let request = ActionExecutionRequest {
            pack_id: args.pack_id,
            file: args.file.clone(),
            bindings: args.bindings,
        };
        let app_data = self.provider_app_data()?;
        let pack = env_provider::action_pack::prepare(service.root(), &app_data, &request)
            .map_err(action_pack_error)?;
        let keys = request
            .bindings
            .values()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        self.store_plan(
            &service,
            PlannedOperation::ActionPack(request),
            format!(
                "{} Action을 {}의 환경변수 {}개로 값 노출 없이 실행합니다.",
                pack.display_name,
                args.file,
                keys.len()
            ),
            vec![args.file],
            keys,
            "opaque-action-pack",
            None,
        )
    }

    pub(super) fn compare_deployment_values(
        &self,
        args: CompareDeploymentValuesArgs,
    ) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let file = args.file.clone();
        let keys = args.keys.clone();
        let comparison = env_provider::provider_push::compare(
            &service,
            ProviderCompareRequest {
                provider: args.provider,
                file: args.file,
                keys: args.keys,
                aws_profile: args.aws_profile,
                aws_region: args.aws_region,
                aws_path_prefix: args.aws_path_prefix,
                runtime_target_id: args.runtime_target_id,
            },
        )
        .map_err(provider_error);
        let result_code = comparison
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        self.audit(
            service.project_id(),
            "compare_deployment_values",
            std::slice::from_ref(&file),
            &keys,
            "opaque-provider-compare",
            result_code,
        );
        let comparison = comparison?;
        serde_json::to_value(comparison).map_err(EnvError::serialization)
    }
}

pub(super) fn provider_error(error: env_provider::provider_push::ProviderPushError) -> EnvError {
    EnvError::invalid(format!("{}: {}", error.code, error.message))
}

pub(super) fn action_pack_error(error: env_provider::action_pack::ActionPackError) -> EnvError {
    EnvError::invalid(format!("{}: {}", error.code, error.message))
}
