#[macro_use] extern crate log;

pub mod runtime;
pub mod gen_queries;
pub mod gen_ts;
pub mod rulesets;

pub use codegen::{queries, deadpool_postgres as deadpool, tokio_postgres as postgres};

pub type PgPool = deadpool::Pool;
pub type PgClient = deadpool::Client;
pub type PgConfig = postgres::Config;

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

pub fn format_ruleset_role(full_path: &str, role_type: RoleType) -> String {
	format!("role:{full_path}|{role_type}")
}


#[derive(Copy, Clone, Debug)]
pub enum ScheduledActionKind { /*Recurring,*/ DetachedRecurring, DetachedScheduled }
impl std::fmt::Display for ScheduledActionKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			// ScheduledActionKind::Recurring => write!(f, "Recurring"),
			ScheduledActionKind::DetachedRecurring => write!(f, "DetachedRecurring"),
			ScheduledActionKind::DetachedScheduled => write!(f, "DetachedScheduled"),
		}
	}
}


pub fn generate_votebase_server_pass() -> String {
	#[cfg(debug_assertions)]
	let votebase_server_password = "votebase_server_dev_pass".to_string();
	#[cfg(not(debug_assertions))]
	let votebase_server_password = {
		use base64::Engine;
		use rand::{Rng, SeedableRng};
		let mut random_bytes = [0u8; 526];
		let mut rng = rand::rngs::StdRng::from_os_rng();
		rng.fill(&mut random_bytes);
		base64::prelude::BASE64_STANDARD.encode(random_bytes)
	};

	votebase_server_password
}

pub async fn load_votebase_server_schema(db_name: String, client: &mut postgres::Client) -> Result<String, AnyError> {
	let votebase_server_password = generate_votebase_server_pass();
	let schema_sql = format!(include_str!("../schema.sql"), db_name=db_name, votebase_server_password=votebase_server_password);
	client.batch_execute(&schema_sql).await?;

	Ok(votebase_server_password)
}
