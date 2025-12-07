pub mod runtime;
pub mod gen_queries;
pub mod gen_ts;

pub use db_generated::{queries, types as db_types, deadpool_postgres as deadpool, tokio_postgres as postgres};

pub type PgConfig = postgres::Config;
pub type PgClient = postgres::Client;
pub type PgPool = deadpool::Pool;

struct FnServerPg(PgPool);
impl From<PgPool> for FnServerPg {
	fn from(value: PgPool) -> Self { FnServerPg(value) }
}
impl std::ops::Deref for FnServerPg {
	type Target = PgPool;
	fn deref(&self) -> &Self::Target { &self.0 }
}
impl std::ops::DerefMut for FnServerPg {
	fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

#[derive(Clone)]
struct FnRolePg {
	config: PgConfig,
	// ruleset_path: String,
}
impl FnRolePg {
	// fn new(config: PgConfig, ruleset_path: String) -> Self {
	// 	FnRolePg { config, ruleset_path }
	// }

	// fn into_role(self, role_type: RoleType, role_pass: &str) -> Self {
	// 	let role_config = make_role_config(&self.ruleset_path, self.config, role_type, role_pass);
	// 	FnRolePg { config: role_config, ruleset_path: self.ruleset_path }
	// }
	fn for_role(ruleset_path: &str, base_config: &PgConfig, role_type: RoleType, role_pass: &str) -> Self {
		let role_config = make_role_config(&ruleset_path, base_config, role_type, role_pass);
		FnRolePg { config: role_config }
	}

	// TODO we're connecting every time here, which seems necessary for security, but terrible for performance
	async fn get_client(&self) -> Result<postgres::Client, deadpool::PoolError> {
		let (client, connection) = self.config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });
		Ok(client)
	}
}

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


pub fn url_encoded_connection_string(config: &PgConfig) -> String {
	use percent_encoding::{utf8_percent_encode, percent_encode, NON_ALPHANUMERIC};
	let mut url = String::from("postgresql://");

	if let Some(user) = config.get_user() {
		url.push_str(&utf8_percent_encode(user, NON_ALPHANUMERIC).to_string());
	}

	if let Some(password_bytes) = config.get_password() {
		url.push(':');
		url.push_str(&percent_encode(password_bytes, NON_ALPHANUMERIC).to_string());
	}

	let hosts = config.get_hosts();
	let ports = config.get_ports();
	if !hosts.is_empty() {
		url.push('@');
		for (i, host) in hosts.iter().enumerate() {
			if i > 0 {
				url.push(',');
			}
			use postgres::config::Host;
			match host {
				Host::Tcp(hostname) => {
					url.push_str(&utf8_percent_encode(hostname, NON_ALPHANUMERIC).to_string());
				}
				#[cfg(unix)]
				Host::Unix(path) => {
					use std::os::unix::ffi::OsStrExt;
					let path_bytes = path.as_os_str().as_bytes();
					url.push_str(&percent_encode(&path_bytes, NON_ALPHANUMERIC).to_string());
				}
			}

			if let Some(&port) = ports.get(i) {
				url.push(':');
				url.push_str(&port.to_string());
			}
		}
	}

	if let Some(dbname) = config.get_dbname() {
		url.push('/');
		url.push_str(&utf8_percent_encode(dbname, NON_ALPHANUMERIC).to_string());
	}

	url
}

#[test]
fn test_url_encoded_connection_string() {
	let config = "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db".parse().unwrap();
	assert_eq!(url_encoded_connection_string(&config), "postgresql://dev_admin_user:dev_admin_password@localhost:5432/dev_db");
}

// #[derive(thiserror::Error, Debug)]
// pub enum ConnectionStringError {
// 	#[error("no user on config?")]
// 	NoUser,
// 	#[error("no host on config?")]
// 	NoHost,
// 	#[error("no port on config?")]
// 	NoPort,
// 	#[error("no dbname on config?")]
// 	NoDbname,
// }


// fn url_encoded_connection_string(config: &Config) -> Result<String, ConnectionStringError> {
// 	use percent_encoding::{utf8_percent_encode, percent_encode, NON_ALPHANUMERIC};

// 	let mut url = String::from("postgresql://");

// 	let user = config.get_user().ok_or(ConnectionStringError::NoUser)?;
// 	url.push_str(&utf8_percent_encode(user, NON_ALPHANUMERIC).to_string());

// 	if let Some(password_bytes) = config.get_password() {
// 		url.push(':');
// 		url.push_str(&percent_encode(password_bytes, NON_ALPHANUMERIC).to_string());
// 	}

// 	let host = config.get_hosts().get(0).ok_or(ConnectionStringError::NoHost)?;
// 	url.push('@');
// 	use votebase_common::postgres::config::Host;
// 	match host {
// 		Host::Tcp(hostname) => {
// 			url.push_str(&utf8_percent_encode(hostname, NON_ALPHANUMERIC).to_string());
// 		}
// 		#[cfg(unix)]
// 		Host::Unix(path) => {
// 			use std::os::unix::ffi::OsStrExt;
// 			let path_bytes = path.as_os_str().as_bytes();
// 			url.push_str(&percent_encode(&path_bytes, NON_ALPHANUMERIC).to_string());
// 		}
// 	}

// 	let port = config.get_ports().get(0).ok_or(ConnectionStringError::NoPort)?;
// 	url.push(':');
// 	url.push_str(&port.to_string());

// 	let dbname = config.get_dbname().ok_or(ConnectionStringError::NoDbname)?;
// 	url.push('/');
// 	url.push_str(&utf8_percent_encode(dbname, NON_ALPHANUMERIC).to_string());

// 	Ok(url)
// }
