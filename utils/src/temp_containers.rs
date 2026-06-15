use std::{io, marker::PhantomData, ops::{Deref, DerefMut}, future::Future};
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
// 		Err(io::Error::other(format!("`podman` couldn't {action}: {err}")))
// 	}
// }

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

async fn wait_for_podman_port(
	container_name: &str,
	max_retries: u64,
	ms_per_retry: u64,
) -> io::Result<u16> {
	for _ in 0..max_retries {
		let podman_port_output = tokio::process::Command::new("podman")
			.args(["port", container_name, "5432/tcp"])
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.spawn()?.wait_with_output().await?;

		if podman_port_output.status.success() {
			let stdout = podman_port_output.stdout;
			if let Some(index) = stdout.iter().position(|&b| b == b':') {
				if let Ok(port) = String::from_utf8_lossy(&stdout[index + 1..]).trim().parse::<u16>() {
					return Ok(port);
				}
			}
		}
		tokio::time::sleep(std::time::Duration::from_millis(ms_per_retry)).await;
	}

	Err(io::Error::other(format!("timed out waiting for podman port for container {}", container_name)))
}

async fn healthcheck_postgres(
	config: &Config,
	max_retries: u64,
	ms_per_retry: u64,
) -> io::Result<()> {
	let slow_threshold = 10 + max_retries / 10;

	for nb_retries in 1..=max_retries {
		if is_postgres_healthy(config).await {
			return Ok(());
		}

		tokio::time::sleep(std::time::Duration::from_millis(ms_per_retry)).await;

		if nb_retries % slow_threshold == 0 {
			eprintln!(
				"Container startup slower than expected ({nb_retries} retries out of {max_retries})"
			);
		}
	}

	Err(io::Error::other("reached the max number of connection retries while waiting for postgres"))
}


pub struct TempPodmanNetwork {
	pub network_name: String,
}
impl TempPodmanNetwork {
	pub async fn new(network_name: String) -> std::io::Result<TempPodmanNetwork> {
		tokio::process::Command::new("podman").args(&["network", "create", &network_name])
			.stderr(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.spawn()?.wait_with_output().await?;

		Ok(TempPodmanNetwork { network_name: network_name.to_string() })
	}

	pub async fn pg(&self) -> std::io::Result<NetworkBoundPg<'_>> {
		let container_name = format!("temp_postgres_{}", random_string(20));
		let pg = TempPodmanPg::new(container_name, Some(&self.network_name)).await?;
		Ok(NetworkBoundPg {
			pg,
			_phantom: PhantomData,
		})
	}
}

pub struct NetworkBoundPg<'a> {
	pub pg: TempPodmanPg,
	_phantom: PhantomData<&'a TempPodmanNetwork>,
}

impl<'a> Deref for NetworkBoundPg<'a> {
	type Target = TempPodmanPg;
	fn deref(&self) -> &Self::Target {
		&self.pg
	}
}

impl<'a> DerefMut for NetworkBoundPg<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pg
	}
}

impl Drop for TempPodmanNetwork {
	fn drop(&mut self) {
		let _ = std::process::Command::new("podman")
			.args(&["network", "rm", "-f", &self.network_name])
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null())
			.status();
	}
}

/// A running podman postgres container with the correct host post discovered.
pub struct TempPodmanPg {
	_child: crate::tokio_graceful_spawn::GracefulChild,
	pub container_name: String,
	pub host_port: u16,
}

pub enum AccessSource {
	FromHost,
	FromPod,
}

pub struct TempClient<'a> {
	client: postgres::Client,
	_phantom: std::marker::PhantomData<&'a TempPodmanPg>,
}

impl<'a> std::ops::Deref for TempClient<'a> {
	type Target = postgres::Client;
	fn deref(&self) -> &Self::Target {
		&self.client
	}
}

impl<'a> std::ops::DerefMut for TempClient<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.client
	}
}

pub struct TempPool<'a> {
	pool: deadpool::Pool,
	_phantom: std::marker::PhantomData<&'a TempPodmanPg>,
}

impl<'a> std::ops::Deref for TempPool<'a> {
	type Target = deadpool::Pool;
	fn deref(&self) -> &Self::Target {
		&self.pool
	}
}

impl<'a> std::ops::DerefMut for TempPool<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pool
	}
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
		Self::new(random_container_name, None)
	}

	// TODO need to audit this implementation
	pub async fn new(
		container_name: String,
		network: Option<&str>,
	) -> std::io::Result<TempPodmanPg> {
		use crate::tokio_graceful_spawn::GracefulSpawn;
		let mut args = vec![
			"run",
			"--name", &container_name,
			"--env", "POSTGRES_DB=temppg_db",
			"--env", "POSTGRES_USER=temppg_admin",
			"--env", "POSTGRES_PASSWORD=temppg_admin_pass",
			"--env", "PGPORT=5432",
			"-p", "127.0.0.1::5432",
			"--rm",
		];
		if let Some(net) = network {
			args.push("--network");
			args.push(net);
		}
		args.push("docker.io/library/postgres:18-alpine");

		let child = tokio::process::Command::new("podman")
			.args(args)
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped())
			.graceful_spawn()?;
			// .debug_output()

		let host_port = wait_for_podman_port(&container_name, 50, 100).await?;

		let temp_podman_pg = TempPodmanPg { _child: child, host_port, container_name };
		let config = temp_podman_pg.make_config(AccessSource::FromHost);
		healthcheck_postgres(&config, 50, 100).await?;

		Ok(temp_podman_pg)
	}

	pub async fn client(&self) -> ContainerResult<TempClient<'_>> {
		let config = self.make_config(AccessSource::FromHost);
		let (client, connection) = config.connect(postgres::NoTls).await?;
		tokio::spawn(async move {
			if let Err(e) = connection.await {
				eprintln!("connection error: {}", e);
			}
		});
		Ok(TempClient {
			client,
			_phantom: std::marker::PhantomData,
		})
	}

	pub fn pool(&self, max_size: usize) -> ContainerResult<TempPool<'_>> {
		let config = self.make_config(AccessSource::FromHost);
		let pool = deadpool::Pool::builder(deadpool::Manager::new(config, postgres::NoTls))
			.max_size(max_size)
			.build()?;
		Ok(TempPool {
			pool,
			_phantom: std::marker::PhantomData,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	async fn assert_no_temp_postgres() -> ContainerResult<()> {
		let pattern = "temp_postgres_";

		tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

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

		let output = tokio::process::Command::new("podman")
			.args(["network", "ls", "--format", "{{.Name}}"]).output().await?;

		assert!(output.status.success(), "podman network ls failed: {}", String::from_utf8_lossy(&output.stderr));

		let stdout = String::from_utf8_lossy(&output.stdout);
		let networks: Vec<&str> = stdout.lines().filter(|line| !line.trim().is_empty()).collect();
		assert_eq!(networks, vec!["podman"], "Expected only the default 'podman' network, but found: {:?}", networks);

		Ok(())
	}

	#[tokio::test]
	async fn test_temp_podman_pg_client() -> ContainerResult<()> {
		let pg = TempPodmanPg::random_new().await?;
		let client = pg.client().await?;

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
		drop(pg);

		assert_no_temp_postgres().await
	}

	#[tokio::test]
	async fn test_temp_podman_pg_pool() -> ContainerResult<()> {
		let pg = TempPodmanPg::random_new().await?;
		let pool = pg.pool(1)?;
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
		drop(pg);

		assert_no_temp_postgres().await
	}

	// TODO need to audit this test
	#[tokio::test]
	async fn test_temp_podman_network_diff() -> ContainerResult<()> {
		let network = TempPodmanNetwork::new(format!("temp_net_{}", random_string(10))).await?;
		let pg1 = network.pg().await?;
		let pg2 = network.pg().await?;

		let client1 = pg1.client().await?;
		client1.execute("CREATE TABLE test_diff (id SERIAL PRIMARY KEY)", &[]).await?;

		let config1 = pg1.make_config(AccessSource::FromPod);
		let config2 = pg2.make_config(AccessSource::FromPod);

		let url1 = crate::url_encoded_connection_string(&config1);
		let url2 = crate::url_encoded_connection_string(&config2);

		let output = podman_run(
			"votebase-dbdiff",
			&["--network", &network.network_name],
			&[
				"--with-privileges",
				&url2, // empty
				&url1, // has table
			]
		).await?;

		let stdout = String::from_utf8_lossy(&output.stdout);
		let stderr = String::from_utf8_lossy(&output.stderr);

		if !output.status.success() && output.status.code() != Some(2) {
			panic!("dbdiff failed with status {}:\nSTDOUT:\n{}\nSTDERR:\n{}", output.status, stdout, stderr);
		}

		assert!(
			stdout.to_lowercase().contains("create table \"public\".\"test_diff\""),
			"Expected diff to contain create table \"public\".\"test_diff\", got: {}", stdout,
		);

		drop(network);

		assert_no_temp_postgres().await
	}
}

