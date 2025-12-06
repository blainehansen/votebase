pub mod runtime;
pub mod gen_queries;
pub mod gen_ts;

pub use db_generated::{queries, types as db_types, deadpool_postgres as deadpool, tokio_postgres as postgres};

pub type PgConfig = postgres::Config;
pub type PgClient = postgres::Client;

pub(crate) struct FnServerPg {
	server_role_config: PgConfig,
	server_role_pool: deadpool::Pool,
	client: Option<deadpool::Client>,
}

impl FnServerPg {
	fn new(server_role_config: PgConfig, server_role_pool: deadpool::Pool) -> Self {
		FnServerPg { server_role_config, server_role_pool, client: None }
	}

	async fn get_client(&mut self) -> Result<&postgres::Client, deadpool::PoolError> {
		if let Some(ref client) = self.client { Ok(client) }
		else {
			Ok(self.client.insert(self.server_role_pool.get().await?))
		}
	}

	// async fn get_mut_client(&mut self) -> Result<&mut postgres::Client, deadpool::PoolError> {
	// 	let client = &mut self.client;
	// 	// TODO would prefer mut ref https://github.com/rust-lang/rust/issues/123076
	// 	if let Some(client) = client { Ok(client) }
	// 	else {
	// 		Ok(client.insert(self.server_role_pool.get().await?))
	// 	}
	// }
}

pub(crate) struct FnRolePg {
	config: PgConfig,
	client: Option<postgres::Client>,
}

impl FnRolePg {
	fn new(config: &PgConfig, full_path: &str, role_type: RoleType, role_pass: &str) -> Self {
		let role_config = make_role_config(full_path, config, role_type, role_pass);
		FnRolePg { config: role_config, client: None }
	}

	async fn get_client(&mut self) -> Result<&postgres::Client, deadpool::PoolError> {
		if let Some(ref client) = self.client { Ok(client) }
		else {
			let (new_client, connection) = self.config.connect(postgres::NoTls).await?;
			tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });
			Ok(self.client.insert(new_client))
		}
	}

	// async fn get_mut_client(&mut self) -> Result<&mut postgres::Client, deadpool::PoolError> {
	// 	let client = &mut self.client;
	// 	// TODO would prefer mut ref https://github.com/rust-lang/rust/issues/123076
	// 	if let Some(client) = client { Ok(client) }
	// 	else {
	// 		let (new_client, connection) = self.config.connect(postgres::NoTls).await?;
	// 		tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });
	// 		Ok(client.insert(new_client))
	// 	}
	// }
}


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
