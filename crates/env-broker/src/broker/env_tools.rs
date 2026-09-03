use super::super::*;

impl Broker {
    pub(super) fn read_allowed(&self, args: ValueArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let value = service.read_allowed_value(&args.file, &args.key);
        let code = value
            .as_ref()
            .map_or_else(|error| error.code().as_str(), |_| "OK");
        self.audit(
            service.project_id(),
            "read_allowed_value",
            std::slice::from_ref(&args.file),
            std::slice::from_ref(&args.key),
            "policy-checked",
            code,
        );
        Ok(json!({ "value": value? }))
    }

    pub(super) fn plan_value(&self, args: PlanValueArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        if service.codex_access(&args.key)? != CodexAccess::ReadWrite {
            let error = EnvError::access_blocked(&args.key);
            self.audit(
                service.project_id(),
                "plan_set_allowed_value",
                std::slice::from_ref(&args.file),
                std::slice::from_ref(&args.key),
                "blocked-by-policy",
                error.code().as_str(),
            );
            return Err(error);
        }
        self.store_plan(
            &service,
            PlannedOperation::SetAllowedValue(SaveValueRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                new_value: args.new_value,
            }),
            format!("{}의 값을 정책 허용 범위에서 교체합니다.", args.key),
            vec![args.file],
            vec![args.key],
            "value-write",
            None,
        )
    }

    pub(super) fn plan_stdin_value(&self, args: PlanStdinValueArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let actor = self
            .agent_host
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .unwrap_or("unknown-agent")
            .to_owned();
        let projection = stdin_value::create_plan(
            &self.provider_app_data()?,
            &service,
            &args.file,
            &args.key,
            args.trim_final_newline,
            &actor,
            &std::env::current_exe()
                .map_err(|error| EnvError::io(Path::new("env-manager-broker"), error))?,
        )?;
        self.audit(
            service.project_id(),
            "create_stdin_value_plan",
            &projection.affected_files,
            &projection.keys,
            projection.risk,
            "OK",
        );
        serde_json::to_value(projection).map_err(EnvError::serialization)
    }

    pub(super) fn plan_create_env_file(
        &self,
        args: PlanCreateEnvFileArgs,
    ) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::CreateEnvFile(CreateEnvFileRequest {
                file: args.file.clone(),
            }),
            format!("{} 빈 env 파일을 만듭니다.", args.file),
            vec![args.file],
            Vec::new(),
            "file-create",
            None,
        )
    }

    pub(super) fn plan_add_variable(&self, args: PlanAddVariableArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::AddVariable(AddVariableRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                group: args.group,
                description: args.description,
                value: String::new(),
            }),
            format!("{} 빈 변수를 추가합니다.", args.key),
            vec![args.file],
            vec![args.key],
            "structural-write",
            None,
        )
    }

    pub(super) fn plan_create_group(&self, args: PlanCreateGroupArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::CreateGroup(CreateGroupRequest {
                file: args.file.clone(),
                name: args.name.clone(),
            }),
            format!("{} 그룹을 만듭니다.", args.name),
            vec![args.file],
            Vec::new(),
            "structural-write",
            None,
        )
    }

    pub(super) fn plan_rename_group(&self, args: PlanRenameGroupArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::RenameGroup(RenameGroupRequest {
                file: args.file.clone(),
                current_name: args.current_name.clone(),
                new_name: args.new_name.clone(),
            }),
            format!(
                "{} 그룹 이름을 {}로 바꿉니다.",
                args.current_name, args.new_name
            ),
            vec![args.file],
            Vec::new(),
            "structural-write",
            None,
        )
    }

    pub(super) fn plan_move_variable(&self, args: PlanMoveVariableArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::MoveVariable(MoveVariableRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                target_group: args.target_group.clone(),
            }),
            format!(
                "{} 변수를 {} 그룹으로 옮깁니다.",
                args.key, args.target_group
            ),
            vec![args.file],
            vec![args.key],
            "structural-write",
            None,
        )
    }

    pub(super) fn plan_update_description(
        &self,
        args: PlanDescriptionArgs,
    ) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::UpdateDescription(SaveDescriptionRequest {
                file: args.file.clone(),
                key: args.key.clone(),
                lines: args.lines,
            }),
            format!("{} 변수 설명을 변경합니다.", args.key),
            vec![args.file],
            vec![args.key],
            "structural-write",
            None,
        )
    }

    pub(super) fn plan_link(&self, args: PlanLinkArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::Link(LinkRequest {
                key: args.key.clone(),
                files: args.files.clone(),
                source_file: args.source_file,
            }),
            format!(
                "{} occurrence {}개를 peer link로 연결합니다.",
                args.key,
                args.files.len()
            ),
            args.files,
            vec![args.key],
            "multi-file-write",
            None,
        )
    }

    pub(super) fn plan_detach(&self, args: PlanDetachArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::Detach {
                link_id: args.link_id,
                file: args.file.clone(),
            },
            "현재 occurrence를 연결에서 분리하고 값은 유지합니다.".to_owned(),
            vec![args.file],
            Vec::new(),
            "relationship-change",
            None,
        )
    }

    pub(super) fn plan_classification(
        &self,
        args: PlanClassificationArgs,
    ) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        self.store_plan(
            &service,
            PlannedOperation::Classification {
                key: args.key.clone(),
                access: args.access,
            },
            format!("{}의 Codex 접근 정책을 변경합니다.", args.key),
            Vec::new(),
            vec![args.key],
            if args.access == CodexAccess::ReadWrite {
                "protection-downgrade"
            } else {
                "policy-change"
            },
            None,
        )
    }

    pub(super) fn plan_migration(&self, args: PlanMigrationArgs) -> Result<Value, EnvError> {
        let service = self.open_registered(&args.project_path)?;
        let migration = service.plan_migration(&args.file)?;
        let preview = migration.preview.clone();
        self.store_plan(
            &service,
            PlannedOperation::Migration(migration),
            preview.summary.clone(),
            vec![args.file],
            Vec::new(),
            "structural-write",
            Some(preview),
        )
    }
}
