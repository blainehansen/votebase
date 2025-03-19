pub mod runtime;

pub type PgPool = sqlx::Pool<sqlx::Postgres>;
pub type PgOpt = sqlx::postgres::PgConnectOptions;

#[derive(Copy, Clone, Debug)]
pub enum RoleType { Migrator, Action, View }
impl std::fmt::Display for RoleType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			RoleType::Migrator => write!(f, "migrator"),
			RoleType::Action => write!(f, "action"),
			RoleType::View => write!(f, "view"),
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub enum FnType { Action, View }
impl std::fmt::Display for FnType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FnType::Action => write!(f, "action"),
			FnType::View => write!(f, "view"),
		}
	}
}

pub fn format_ruleset_schema(full_path: &str) -> String {
	format!("ruleset:{full_path}")
}

pub fn format_ruleset_role(full_path: &str, fn_type: RoleType) -> String {
	format!("role:{full_path}|{fn_type}")
}
