pub const SCHEMA_VERSION: i64 = 9;
pub const INIT_SQL: &str = include_str!("../../migrations/001_init.sql");
pub const MIGRATION_002_SQL: &str = include_str!("../../migrations/002_templates.sql");
pub const MIGRATION_003_SQL: &str = include_str!("../../migrations/003_template_history.sql");
pub const MIGRATION_004_SQL: &str = include_str!("../../migrations/004_run_session_standalone.sql");
pub const MIGRATION_005_SQL: &str = include_str!("../../migrations/005_command_history.sql");
pub const MIGRATION_006_SQL: &str = include_str!("../../migrations/006_shell_history.sql");
pub const MIGRATION_007_SQL: &str =
    include_str!("../../migrations/007_history_privacy_patterns.sql");
pub const MIGRATION_008_SQL: &str = include_str!("../../migrations/008_command_shell_kind.sql");
pub const MIGRATION_009_SQL: &str = include_str!("../../migrations/009_group_execution_mode.sql");
