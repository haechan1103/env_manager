mod discovery;
mod effective;
mod error;
mod export;
mod git_safety;
mod manifest;
mod migration;
mod model;
mod parser;
mod policy;
mod provider_push;
mod service;
mod team_import;
mod transaction;

pub use discovery::{DiscoveryOptions, discover_env_files};
pub use effective::{
    EffectiveContext, EffectiveOccurrence, EffectiveProjection, FrameworkKind, resolve_effective,
};
pub use error::{EnvError, EnvErrorCode, EnvResult};
pub use export::{ExportFormat, ExportOccurrence, ExportSummary, export_project_env};
pub use git_safety::{
    GitSafetyProjection, GitSafetyState, GitignoreUpdateSummary, apply_gitignore_guard,
    inspect_git_safety,
};
pub use manifest::{
    ClassificationSource, CodexAccess, LinkGroup, LinkMember, MANIFEST_FILE_NAME, Manifest,
    ManifestStore, VariablePolicy, validate_display_name,
};
pub use migration::{MigrationPlan, MigrationPreview, MigrationSuggestion};
pub use model::{
    ClassificationReviewProjection, FileProjection, GroupProjection, OccurrenceProjection,
    ProjectProjection, RedactedValueState,
};
pub use parser::{AssignmentRef, Document, NewlineStyle, Node, Span};
pub use policy::{
    ClassificationSuggestion, ClientExposureWarning, detect_client_exposure, suggest_access,
};
pub use provider_push::ProviderValue;
pub use service::{
    AddVariableRequest, CreateEnvFileRequest, CreateGroupRequest, DeleteVariableRequest,
    LinkRequest, MoveVariableRequest, MutationSummary, OpaqueValueCopyRequest, ProjectService,
    RedactedOccurrenceReference, RenameGroupRequest, SaveDescriptionRequest, SaveValueRequest,
};
pub use team_import::{
    TeamImportFileProjection, TeamImportOccurrenceProjection, TeamImportOccurrenceState,
    TeamImportPlan, TeamImportPreview, TeamImportSummary, TeamImportValueSide,
    plan_encrypted_team_import,
};
pub use transaction::{FileRevision, PlannedFileChange, TransactionPlan};
