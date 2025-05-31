async fn is_installed(tool: &str) -> bool {
	let status = tokio::process::Command::new(tool)
		.arg("--version")
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.status().await;

	status.is_ok() && status.unwrap().success()
}

pub async fn setup(podman: bool) -> Result<(), ContainerError> {
	let command = if podman { "podman" } else { "docker" };
	if !is_installed(command).await {
		return Err(ContainerError(format!("`{command}` is not installed or not found in PATH.")));
	}

	spawn_container(podman).await?;
	healthcheck(podman, 120, 50).await?;
	Ok(())
}

pub async fn cleanup(podman: bool) -> Result<(), ContainerError> {
	stop_container(podman).await?;
	remove_container(podman).await?;
	Ok(())
}

async fn spawn_container(podman: bool) -> Result<(), ContainerError> {
	cmd(
		// POSTGRES_DB=dev_db
		// POSTGRES_USER=dev_admin_user
		// POSTGRES_PASSWORD=dev_admin_password
		podman,
		&[
			"run",
			"-d",
			"--name",
			"votebase_postgres",
			"-p",
			"5435:5432",
			"-e",
			"POSTGRES_PASSWORD=postgres",
			"docker.io/library/postgres:latest",
		],
		"spawn container",
	).await
}

async fn is_postgres_healthy(podman: bool) -> Result<bool, ContainerError> {
	Ok(cmd(
		podman,
		&["exec", "votebase_postgres", "pg_isready"],
		"check container health",
	).await.is_ok())
}

async fn healthcheck(podman: bool, max_retries: u64, ms_per_retry: u64) -> Result<(), ContainerError> {
	let slow_threshold = 10 + max_retries / 10;
	let mut nb_retries = 0;
	while !is_postgres_healthy(podman).await? {
		if nb_retries >= max_retries {
			return Err(ContainerError(String::from("Clorinde reached the max number of connection retries")));
		};
		tokio::time::sleep(std::time::Duration::from_millis(ms_per_retry)).await;
		nb_retries += 1;

		if nb_retries % slow_threshold == 0 {
			println!(
				"Container startup slower than expected ({nb_retries} retries out of {max_retries})"
			);
		}
	}
	tokio::time::sleep(std::time::Duration::from_millis(250)).await;
	Ok(())
}

async fn stop_container(podman: bool) -> Result<(), ContainerError> {
	cmd(podman, &["stop", "votebase_postgres"], "stop container").await
}

async fn remove_container(podman: bool) -> Result<(), ContainerError> {
	cmd(
		podman,
		&["rm", "-v", "votebase_postgres"],
		"remove container",
	).await
}

async fn cmd(podman: bool, args: &[&'static str], action: &'static str) -> Result<(), ContainerError> {
	let command = if podman { "podman" } else { "docker" };
	let output = tokio::process::Command::new(command)
		.args(args)
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::null())
		.output().await?;

	if output.status.success() {
		Ok(())
	} else {
		let err = String::from_utf8_lossy(&output.stderr);
		Err(ContainerError(format!("`{command}` couldn't {action}: {err}")))
	}
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ContainerError(String);

impl From<std::io::Error> for ContainerError {
	fn from(e: std::io::Error) -> Self {
		Self(format!("{e:#}"))
	}
}
