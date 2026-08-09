mod discovery;
mod effective;
mod error;
mod git_safety;
mod manifest;
mod migration;
mod model;
mod parser;
mod policy;
mod service;
mod transaction;

pub use discovery::{DiscoveryOptions, discover_env_files};
pub use effective::{
    EffectiveContext, EffectiveOccurrence, EffectiveProjection, FrameworkKind, resolve_effective,
};
pub use error::{EnvError, EnvErrorCode, EnvResult};
pub use git_safety::{
    GitSafetyProjection, GitSafetyState, GitignoreUpdateSummary, apply_gitignore_guard,
    inspect_git_safety,
};
pub use manifest::{
    ClassificationSource, CodexAccess, LinkGroup, LinkMember, MANIFEST_FILE_NAME, Manifest,
    ManifestStore, VariablePolicy,
};
pub use migration::{MigrationPlan, MigrationPreview, MigrationSuggestion};
pub use model::{
    FileProjection, GroupProjection, OccurrenceProjection, ProjectProjection, RedactedValueState,
};
pub use parser::{AssignmentRef, Document, NewlineStyle, Node, Span};
pub use policy::{ClassificationSuggestion, suggest_access};
pub use service::{
    AddVariableRequest, CreateEnvFileRequest, CreateGroupRequest, DeleteVariableRequest,
    LinkRequest, MoveVariableRequest, MutationSummary, ProjectService, RenameGroupRequest,
    SaveDescriptionRequest, SaveValueRequest,
};
pub use transaction::{FileRevision, PlannedFileChange, TransactionPlan};
