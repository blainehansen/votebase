use std::io;
use tokio_postgres::Config;

#[derive(thiserror::Error, Debug)]
pub enum ContainerError {
	#[error(transparent)]
	PostgresError(#[from] tokio_postgres::Error),
	#[error(transparent)]
	StdIoError(#[from] std::io::Error),
}

pub type ContainerResult<T> = Result<T, ContainerError>;


fn random_string(len: usize) -> String {
	let mut rng = rand::rng();
	use rand::distr::SampleString;
	rand::distr::Alphanumeric.sample_string(&mut rng, len)
}

fn random_port() -> u16 {
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

// 	spawn_postgres(container_image).await?;
// 	healthcheck_postgres(120, 50, container_wait).await?;
// 	Ok(())
// }

pub async fn run_workspace_podman_cmd(image: &str, workspace_arg: impl AsRef<str>, args: &[&str]) -> io::Result<std::process::Output> {
	let args = [
		["run", "--rm", "-v", workspace_arg.as_ref(), image].as_slice(),
		args,
	].concat();

	tokio::process::Command::new("podman")
		.args(args)
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await
}

async fn podman_cmd(args: &[&str], action: &'static str) -> io::Result<()> {
	let output = tokio::process::Command::new("podman")
		.args(args)
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::null())
		.spawn()?.wait_with_output().await?;

	if output.status.success() {
		Ok(())
	} else {
		let err = String::from_utf8_lossy(&output.stderr);
		Err(io::Error::new(io::ErrorKind::Other, format!("`podman` couldn't {action}: {err}")))
	}
}

async fn spawn_postgres(container_name: &str, pg_pass: &str, pg_user: &str, pg_db: &str, pg_port: u16) -> io::Result<()> {
	podman_cmd(&[
		"run",
		"--name", &container_name,
		"-e", &format!("POSTGRES_PASSWORD={pg_pass}"),
		"-e", &format!("POSTGRES_USER={pg_user}"),
		"-e", &format!("POSTGRES_DB={pg_db}"),
		"-e", &format!("PGPORT={pg_port}"),
		"-p", &format!("{pg_port}:{pg_port}"),
		"-d",
		"docker.io/library/postgres:latest",
	], "start temp container").await
}

async fn is_postgres_healthy(config: &Config) -> bool {
	// let args = ["exec", container_name, "pg_isready"];
	// Ok(podman_cmd(&args, "check container health").await.is_ok())
	config.connect(tokio_postgres::NoTls).await.is_ok()

	// let res = config.connect(tokio_postgres::NoTls).await;
	// match res {
	// 	Ok(_) => true,
	// 	Err(e) => {
	// 		eprintln!("{:?}", e);
	// 		false
	// 	},
	// }
}

async fn healthcheck_postgres(
	config: &Config,
	max_retries: u64,
	ms_per_retry: u64,
) -> io::Result<()> {
	let slow_threshold = 10 + max_retries / 10;
	let mut nb_retries = 0;

	while !is_postgres_healthy(config).await {
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
	F: FnOnce(Config) -> Fut,
>(func: F) -> ContainerResult<Fut::Output> {
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = generate_temp_config();

	kill_container(&container_name).await?;

	spawn_postgres(&container_name, &pg_pass, &pg_user, &pg_db, pg_port).await?;
	healthcheck_postgres(&config, 50, 100).await?;

	// call user function
	let result = func(config).await;

	kill_container(&container_name).await?;

	Ok(result)
}

pub async fn with_temp_postgres_client<
	Fut: Future,
	F: FnOnce(Config, tokio_postgres::Client) -> Fut,
>(func: F) -> ContainerResult<Fut::Output> {
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = generate_temp_config();

	kill_container(&container_name).await?;

	spawn_postgres(&container_name, &pg_pass, &pg_user, &pg_db, pg_port).await?;
	healthcheck_postgres(&config, 50, 100).await?;

	// call user function
	let (client, connection) = config.connect(tokio_postgres::NoTls).await?;
	tokio::spawn(async move {
		if let Err(e) = connection.await {
			eprintln!("connection error: {}", e);
		}
	});
	let result = func(config, client).await;

	kill_container(&container_name).await?;

	Ok(result)
}

fn generate_temp_config() -> (String, Config, String, String, String, u16) {
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

	(container_name, config, pg_pass.to_string(), pg_user.to_string(), pg_db.to_string(), pg_port)
}

async fn kill_container(container_name: &str) -> io::Result<()> {
	podman_cmd(&["stop", "--ignore", &container_name], "stop container").await?;
	podman_cmd(&["rm", "-v", "--ignore", &container_name], "cleanup container").await?;
	Ok(())
}


#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_postgres_with_process_setup() -> ContainerResult<()> {
		with_temp_postgres(async |config| -> ContainerResult<()> {
			let (client, connection) = config.connect(tokio_postgres::NoTls).await?;

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

		with_temp_postgres_client(async |_, client| -> ContainerResult<()> {
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

		Ok(())
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
