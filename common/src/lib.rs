pub mod rulesets;
// #[cfg(test)]
// mod rulesets_test;

pub mod runtime;
pub mod gen_queries;
pub mod gen_ts;
mod podman_fns;

pub use db_generated::{queries, types as db_types, deadpool_postgres as deadpool, tokio_postgres as postgres};

pub type PgConfig = postgres::Config;
pub type PgClient = postgres::Client;
pub type PgPool = deadpool::Pool;


async fn pg_con(config: &PgConfig) -> Result<PgClient, postgres::Error> {
	let (client, conn) = config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = conn.await { log::error!("DB connection error: {}", e); } });
	Ok(client)
}

// #[derive(Clone)]
// struct FnRolePg {
// 	pool: PgPool,
// 	role_name: String,
// 	// ruleset_path: String,
// }
// impl FnRolePg {
// 	// fn new(config: PgConfig, ruleset_path: String) -> Self {
// 	// 	FnRolePg { config, ruleset_path }
// 	// }

// 	// fn into_role(self, role_type: RoleType, role_pass: &str) -> Self {
// 	// 	let role_config = make_role_config(&self.ruleset_path, self.config, role_type, role_pass);
// 	// 	FnRolePg { config: role_config, ruleset_path: self.ruleset_path }
// 	// }
// 	fn for_role(pool: PgPool, ruleset_path: &str, base_config: &PgConfig, role_type: RoleType, role_pass: &str) -> Self {
// 		let role_config = make_role_config(&ruleset_path, base_config, role_type, role_pass);
// 		FnRolePg { config: role_config }
// 	}

// 	// SAFETY this relies on
// 	async fn get_client(&self) -> Result<postgres::Client, deadpool::PoolError> {
// 		let client = self.pool.get().await?;
// 		client.batch_execute(&format!(r#""#)).await?;
// 		Ok(client)
// 	}
// }

// TODO what I think I'm going to do is this:
// *AFTER I'VE IMPLEMENTED THINGS AND THEY'RE WORKING AND THE CONCEPTUAL PROTOTYPE OF VOTEBASE IS DONE AND I'M THINKING ABOUT PERFORMANCE*
// then I'll basically implement a FnPgPool! it could be a fork/inspired by the Manager/Object/etc in deadpool-postgres, so I'll make my own manager, or I might have to used deadpool::unmanaged, or make my own little pool thing that's inspired by it
// what it will do is have a finite number of connections it will make for *all* fn calls, and it will use an async semaphore to guard them
// but when a particular ruleset/role combination connects, I'll put the connection in this pool thing and give it out. what this means is that the `get` method for the pool will have to include a ruleset_path and role_type, so the pool will give a connection of that combination if one's available, and otherwise will close a connection and make a new one with the right info

// in the fullness of time, I'll implement a new LANGUAGE OR DATABASE OR WHATEVER, so that these permissions checks don't happen at the connection level. it makes sense from a server/client architectural perspective that opening an authenticated connection kinda has to be slow, but what should be possible is for a single server role to have a normal connection pool, but then send a statement to the database along with roles and ask *at a type level* if that statement is allowed, without actually trying to execute it. then we only have to secure the "information channel" that passes along the role info, not the statement itself, and that works in the context of votebase because the role etc is determined by what fn is running


// pub(crate) struct FnRolePg {
// 	config: PgConfig,
// 	client: tokio::sync::RwLock<Option<postgres::Client>>,
// 	// client: Option<postgres::Client>,
// }

// impl FnRolePg {

// 	async fn get_client(&self) -> Result<tokio::sync::RwLockReadGuard<postgres::Client>, deadpool::PoolError> {
// 		let client = self.client.read().await;
// 		if client.is_some() {
// 			return Ok(tokio::sync::RwLockReadGuard::map(client, |o| o.as_ref().unwrap()))
// 		}
// 		else {
// 			drop(client);
// 			let (new_client, connection) = self.config.connect(postgres::NoTls).await?;
// 			tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });
// 			let mut client = self.client.write().await;
// 			client.replace(new_client);
// 			Ok(tokio::sync::RwLockWriteGuard::downgrade_map(client, |o| o.as_ref().unwrap()))
// 		}
// 	}
// }


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

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum FnType { Action, View }
impl std::fmt::Display for FnType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FnType::Action => write!(f, "action"),
			FnType::View => write!(f, "view"),
		}
	}
}
impl From<db_types::votebase_catalog::FnType> for FnType {
	fn from(value: db_types::votebase_catalog::FnType) -> Self {
		match value {
			db_types::votebase_catalog::FnType::Action => Self::Action,
			db_types::votebase_catalog::FnType::View => Self::View,
		}
	}
}
impl Into<db_types::votebase_catalog::FnType> for FnType {
	fn into(self) -> db_types::votebase_catalog::FnType {
		match self {
			Self::Action => db_types::votebase_catalog::FnType::Action,
			Self::View => db_types::votebase_catalog::FnType::View,
		}
	}
}


pub fn format_full_path(parent_full_path: Option<&str>, child_name: &str) -> String {
	match parent_full_path {
		Some(parent_full_path) => format!("{parent_full_path}|{child_name}"),
		None => format!("{child_name}"),
	}
}

pub fn split_full_path(full_path: &str) -> (Option<String>, String) {
	if let Some(pos) = full_path.rfind('|') {
		let parent = &full_path[..pos];
		let child = &full_path[pos + 1..];
		(Some(parent.to_string()), child.to_string())
	}
	else {
		(None, full_path.to_string())
	}
}

#[test]
fn test_split_full_path() {
	assert_eq!(split_full_path("a|b|c"), (Some("a|b".to_string()), "c".to_string()));
	assert_eq!(split_full_path("single"), (None, "single".to_string()));
}


/// `ruleset:{full_path}`
pub fn format_ruleset_schema(full_path: &str) -> String {
	format!("ruleset:{full_path}")
}

/// `role:{full_path}|{role_type}`
pub fn format_ruleset_role(full_path: &str, role_type: RoleType) -> String {
	format!("role:{full_path}|{role_type}")
}

fn make_role_config(full_path: &str, config: &PgConfig, role_type: RoleType, role_pass: &str) -> PgConfig {
	let mut role_config = config.clone();
	let formatted_ruleset_role = format_ruleset_role(full_path, role_type);
	role_config.user(formatted_ruleset_role).password(role_pass);
	role_config
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
