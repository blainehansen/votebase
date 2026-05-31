use std::{borrow::Cow, io};
use tokio_postgres::{self as postgres, Config};
use deadpool_postgres as deadpool;

#[derive(thiserror::Error, Debug)]
pub enum ContainerError {
	#[error(transparent)]
	PostgresError(#[from] postgres::Error),
	#[error(transparent)]
	DeadpoolError(#[from] deadpool::PoolError),
	#[error(transparent)]
	DeadpoolBuildError(#[from] deadpool::BuildError),
	#[error(transparent)]
	StdIoError(#[from] std::io::Error),
}

pub type ContainerResult<T> = Result<T, ContainerError>;


pub fn random_string(len: usize) -> String {
	let mut rng = rand::rng();
	use rand::distr::SampleString;
	rand::distr::Alphanumeric.sample_string(&mut rng, len)
}

pub fn random_port() -> u16 {
	let mut rng = rand::rng();
	use rand::Rng;
	rng.random_range(6000..=65535)
}

// async fn is_installed(tool: &str) -> bool {
// 	let status = tokio::process::Command::new(tool)
// 		.arg("--version")
// 		.stdout(std::process::Stdio::null())
// 		.stderr(std::process::Stdio::null())
// 		.status().await;

// 	status.is_ok_and(|s| s.success())
// }

// pub async fn setup(container_image: &str, container_wait: u64) -> ContainerResult<()> {
// 	let command = "podman";
// 	if !is_installed(command).await {
// 		return Err(anyhow::anyhow!("`{command}` is not installed or not found in PATH."));
// 	}

// 	spawn_postgres_tokio(container_image).await?;
// 	healthcheck_postgres(120, 50, container_wait).await?;
// 	Ok(())
// }

pub async fn workspace_podman_run(
	image: &str,
	podman_args: &[&str],
	workspace_arg: impl AsRef<str>,
	args: &[&str],
) -> io::Result<std::process::Output> {
	let args = [
		["run", "--rm"].as_slice(),
		podman_args,
		["-v", workspace_arg.as_ref()].as_slice(),
		[image].as_slice(),
		args,
	].concat();

	tokio::process::Command::new("podman")
		.args(args)
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await
}

pub async fn podman_run(
	image: &str,
	podman_args: &[&str],
	args: &[&str],
) -> io::Result<std::process::Output> {
	let args = [
		["run", "--rm"].as_slice(),
		podman_args,
		[image].as_slice(),
		args,
	].concat();

	tokio::process::Command::new("podman")
		.args(args)
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await
}

// pub async fn podman_cmd(args: &[&str], action: &'static str) -> io::Result<()> {
// 	let output = tokio::process::Command::new("podman")
// 		.args(args)
// 		.stderr(std::process::Stdio::piped())
// 		.stdout(std::process::Stdio::piped())
// 		.spawn()?.wait_with_output().await?;

// 	if output.status.success() {
// 		Ok(())
// 	} else {
// 		let err = String::from_utf8_lossy(&output.stderr);
// 		Err(io::Error::new(io::ErrorKind::Other, format!("`podman` couldn't {action}: {err}")))
// 	}
// }

pub fn spawn_postgres_tokio(
	container_name: &str,
	pg_pass: &str,
	pg_user: &str,
	pg_db: &str,
	pg_port: u16,
) -> io::Result<crate::tokio_graceful_spawn::GracefulChild> {
	use crate::tokio_graceful_spawn::GracefulSpawn;
	let child = tokio::process::Command::new("podman")
		.args([
			"run",
			"--name", container_name,
			"--env", &format!("POSTGRES_PASSWORD={pg_pass}"),
			"--env", &format!("POSTGRES_USER={pg_user}"),
			"--env", &format!("POSTGRES_DB={pg_db}"),
			"--env", &format!("PGPORT={pg_port}"),
			"-p", &format!("{pg_port}:{pg_port}"),
			"--rm",
			"docker.io/library/postgres:latest",
		])
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.graceful_spawn()?;

	Ok(child)
}

pub fn spawn_postgres_std(
	container_name: &str,
	pg_pass: &str,
	pg_user: &str,
	pg_db: &str,
	pg_port: u16,
) -> io::Result<crate::std_graceful_spawn::GracefulChild> {
	use crate::std_graceful_spawn::GracefulSpawn;
	let child = std::process::Command::new("podman")
		.args([
			"run",
			"--name", container_name,
			"--env", &format!("POSTGRES_PASSWORD={pg_pass}"),
			"--env", &format!("POSTGRES_USER={pg_user}"),
			"--env", &format!("POSTGRES_DB={pg_db}"),
			"--env", &format!("PGPORT={pg_port}"),
			"-p", &format!("{pg_port}:{pg_port}"),
			"--rm",
			"docker.io/library/postgres:latest",
		])
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.graceful_spawn()?;

	Ok(child)
}


async fn is_postgres_healthy(config: &Config) -> bool {
	// let args = ["exec", container_name, "pg_isready"];
	// Ok(podman_cmd(&args, "check container health").await.is_ok())
	config.connect(postgres::NoTls).await.is_ok()

	// let res = config.connect(postgres::NoTls).await;
	// match res {
	// 	Ok(_) => true,
	// 	Err(e) => {
	// 		eprintln!("{:?}", e);
	// 		false
	// 	},
	// }
}

pub async fn healthcheck_postgres(
	config: &Config,
	max_retries: u64,
	ms_per_retry: u64,
) -> io::Result<()> {
	let slow_threshold = 10 + max_retries / 10;
	let mut nb_retries = 0;

	while !is_postgres_healthy(&config).await {
		if nb_retries >= max_retries {
			return Err(io::Error::new(io::ErrorKind::Other, "reached the max number of connection retries while waiting for postgres"))
		};

		tokio::time::sleep(std::time::Duration::from_millis(ms_per_retry)).await;
		nb_retries += 1;

		if nb_retries % slow_threshold == 0 {
			eprintln!(
				"Container startup slower than expected ({nb_retries} retries out of {max_retries})"
			);
		}
	}

	Ok(())
}


pub async fn with_temp_postgres<
	Fut: Future,
	F: FnOnce(String, Config) -> Fut,
>(func: F) -> ContainerResult<Fut::Output> {
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = generate_temp_config();

	let _postgres_process = spawn_postgres_tokio(&container_name, &pg_pass, &pg_user, &pg_db, pg_port)?;
	healthcheck_postgres(&config, 50, 100).await?;

	Ok(func(container_name, config).await)
}

pub async fn with_temp_postgres_client<
	Fut: Future,
	F: FnOnce(String, Config, postgres::Client) -> Fut,
>(func: F) -> ContainerResult<Fut::Output> {
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = generate_temp_config();

	let _postgres_process = spawn_postgres_tokio(&container_name, &pg_pass, &pg_user, &pg_db, pg_port)?;
	healthcheck_postgres(&config, 50, 100).await?;

	let (client, connection) = config.connect(postgres::NoTls).await?;
	tokio::spawn(async move {
		if let Err(e) = connection.await {
			eprintln!("connection error: {}", e);
		}
	});

	Ok(func(container_name, config, client).await)
}

pub async fn with_temp_postgres_pool<
	Fut: Future,
	F: FnOnce(String, Config, deadpool::Pool) -> Fut,
>(func: F) -> ContainerResult<Fut::Output> {
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = generate_temp_config();

	let _postgres_process = spawn_postgres_tokio(&container_name, &pg_pass, &pg_user, &pg_db, pg_port)?;
	healthcheck_postgres(&config, 50, 100).await?;

	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(5).build()?;

	Ok(func(container_name, config, pool).await)
}

pub fn generate_temp_config() -> (String, Config, &'static str, &'static str, &'static str, u16) {
	let random_suffix = random_string(20);
	let container_name = format!("temp_postgres_{random_suffix}");
	let pg_pass = "temppass";
	let pg_user = "tempuser";
	let pg_db = "tempdb";
	let pg_port = random_port();

	let mut config = Config::new();
	config.host("localhost");
	config.password(pg_pass);
	config.user(pg_user);
	config.dbname(pg_db);
	config.port(pg_port);

	(container_name, config, pg_pass, pg_user, pg_db, pg_port)
}

#[cfg(test)]
mod tests {
	use super::*;

	async fn assert_no_temp_postgres() -> ContainerResult<()> {
		let pattern = "temp_postgres_";

		tokio::time::sleep(std::time::Duration::from_millis(300)).await;

		let output = tokio::process::Command::new("podman")
			.args(["ps", "-a", "--format", "{{.Names}}"]).output().await?;

		assert!(output.status.success(), "podman ps -a failed: {}", String::from_utf8_lossy(&output.stderr));

		let stdout = String::from_utf8_lossy(&output.stdout);
		let matches: Vec<&str> = stdout
			.lines().filter(|line| line.contains(pattern))
			.collect();

		assert!(
			matches.is_empty(),
			"Expected no containers matching {:?}, but found: {:?}", pattern, matches,
		);

		Ok(())
	}

	#[tokio::test]
	async fn test_with_temp_postgres() -> ContainerResult<()> {
		with_temp_postgres(async |_, config| -> ContainerResult<()> {
			let (client, connection) = config.connect(postgres::NoTls).await?;

			tokio::spawn(async move {
				if let Err(e) = connection.await {
					eprintln!("connection error: {}", e);
				}
			});

			client.execute(
				"CREATE TABLE test_users (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
				&[]
			).await?;

			client.execute(
				"INSERT INTO test_users (name) VALUES ($1), ($2)",
				&[&"Alice", &"Bob"]
			).await?;

			let rows = client.query("SELECT id, name FROM test_users ORDER BY id", &[]).await?;

			assert_eq!(rows.len(), 2);
			assert_eq!(rows[0].get::<_, i32>(0), 1);
			assert_eq!(rows[0].get::<_, &str>(1), "Alice");
			assert_eq!(rows[1].get::<_, &str>(1), "Bob");

			Ok(())
		}).await??;

		assert_no_temp_postgres().await
	}

	#[tokio::test]
	async fn test_with_temp_postgres_client() -> ContainerResult<()> {
		with_temp_postgres_client(async |_, _, client| -> ContainerResult<()> {
			client.execute(
				"CREATE TABLE test_users (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
				&[]
			).await?;

			client.execute(
				"INSERT INTO test_users (name) VALUES ($1), ($2)",
				&[&"Alice", &"Bob"]
			).await?;

			let rows = client.query("SELECT id, name FROM test_users ORDER BY id", &[]).await?;

			assert_eq!(rows.len(), 2);
			assert_eq!(rows[0].get::<_, i32>(0), 1);
			assert_eq!(rows[0].get::<_, &str>(1), "Alice");
			assert_eq!(rows[1].get::<_, &str>(1), "Bob");

			Ok(())
		}).await??;

		assert_no_temp_postgres().await
	}

	#[tokio::test]
	async fn test_with_temp_postgres_pool() -> ContainerResult<()> {
		with_temp_postgres_pool(async |_, _, pool| -> ContainerResult<()> {
			let client = pool.get().await?;

			client.execute(
				"CREATE TABLE test_users (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
				&[]
			).await?;

			client.execute(
				"INSERT INTO test_users (name) VALUES ($1), ($2)",
				&[&"Alice", &"Bob"]
			).await?;

			let rows = client.query("SELECT id, name FROM test_users ORDER BY id", &[]).await?;

			assert_eq!(rows.len(), 2);
			assert_eq!(rows[0].get::<_, i32>(0), 1);
			assert_eq!(rows[0].get::<_, &str>(1), "Alice");
			assert_eq!(rows[1].get::<_, &str>(1), "Bob");

			Ok(())
		}).await??;

		assert_no_temp_postgres().await
	}
}



pub struct PodmanNetwork {
	pub network_name: String,
}
impl PodmanNetwork {
	pub async fn new(network_name: String) -> std::io::Result<PodmanNetwork> {
		tokio::process::Command::new("podman").args(&["network", "create", &network_name])
			.stderr(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.spawn()?.wait_with_output().await?;

		Ok(PodmanNetwork { network_name: network_name.to_string() })
	}
}

impl Drop for PodmanNetwork {
	fn drop(&mut self) {
		let network_name = self.network_name.clone();

		tokio::spawn(async move {
			tokio::process::Command::new("podman").args(&["network", "rm", "-f", &network_name])
			.stderr(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.spawn()?.wait_with_output().await?;

			Ok::<_, std::io::Error>(())
		});
	}
}


// pub(crate) mod error {
// 	use std::fmt::Debug;

// 	use miette::Diagnostic;
// 	use thiserror::Error as ThisError;

// 	#[derive(Debug, ThisError, Diagnostic)]
// 	#[error("{msg}")]
// 	pub struct Error {
// 		pub(crate) msg: String,
// 		#[help]
// 		pub help: Option<String>,
// 	}

// 	impl Error {
// 		pub fn new(msg: String, ) -> Self {
// 			let help = if podman {
// 				"Make sure that port 5435 is usable and that no container named `clorinde_postgres` already exists."
// 			} else {
// 				"First, check that the docker daemon is up-and-running. Then, make sure that port 5435 is usable and that no container named `clorinde_postgres` already exists."
// 			};
// 			Error {
// 				msg,
// 				help: Some(String::from(help)),
// 			}
// 		}
// 	}

// 	impl From<io::Error> for Error {
// 		fn from(e: std::io::Error) -> Self {
// 			Self {
// 				msg: format!("{e:#}"),
// 				help: None,
// 			}
// 		}
// 	}
// }



/// A running podman postgres container with the correct host post discovered.
pub struct TempPodmanPg {
	#[allow(dead_code)]
	child: crate::tokio_graceful_spawn::GracefulChild,
	pub container_name: String,
	pub host_port: u16,
}

pub enum AccessSource {
	FromHost,
	FromPod,
}

impl TempPodmanPg {
	pub const DBNAME: &'static str = "temppg_db";
	pub const USER: &'static str = "temppg_admin";
	pub const PASSWORD: &'static str = "temppg_admin_pass";
	pub const PORT: u16 = 5432;

	pub fn make_config(&self, access_source: AccessSource) -> Config {
		let mut config = Config::new();
		config.dbname(Self::DBNAME);
		config.user(Self::USER);
		config.password(Self::PASSWORD);

		match access_source {
			AccessSource::FromHost => {
				config.host("localhost");
				config.port(self.host_port);
			},
			AccessSource::FromPod => {
				config.host(&self.container_name);
				config.port(Self::PORT);
			},
		}
		config
	}

	pub fn random_new() -> impl Future<Output = std::io::Result<TempPodmanPg>> {
		let random_container_name = format!("temp_postgres_{}", random_string(20));
		Self::new(random_container_name)
	}

	pub async fn new(
		container_name: String,
	) -> std::io::Result<TempPodmanPg> {
		use crate::tokio_graceful_spawn::GracefulSpawn;
		let child = tokio::process::Command::new("podman")
			.args([
				"run",
				"--name", &container_name,
				"--env", "POSTGRES_DB=temppg_db",
				"--env", "POSTGRES_USER=temppg_admin",
				"--env", "POSTGRES_PASSWORD=temppg_admin_pass",
				"--env", "PGPORT=5432",
				"-p", "127.0.0.1::5432",
				"--rm",
				"docker.io/library/postgres:18-alpine",
			])
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.graceful_spawn()?;


		let podman_port_output =
			tokio::process::Command::new("podman").args(["port", &container_name, "5432/tcp"])
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.spawn()?.wait_with_output().await?;

		if !podman_port_output.status.success() {
			let err = String::from_utf8_lossy(&podman_port_output.stderr);
			return Err(io::Error::other(format!("couldn't spawn TempPodmanPg: {err}")))
		}

		let stdout = podman_port_output.stdout;
		let index = stdout.iter().position(|&b| b == b':')
			.ok_or_else(|| io::Error::other(format!("podman port output is malformed {}", String::from_utf8_lossy(&stdout))))?;

		let host_port = String::from_utf8_lossy(&stdout[index..]).parse::<u16>()
			.map_err(io::Error::other)?;
		let temp_podman_pg = TempPodmanPg { child, host_port, container_name };
		let config = temp_podman_pg.make_config(AccessSource::FromHost);
		healthcheck_postgres(&config, 50, 100).await?;

		Ok(temp_podman_pg)
	}
}
