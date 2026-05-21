use crate::{PgConfig};

pub async fn podman_compute_diff(
	podman_network: &utils::temp_containers::PodmanNetwork,
	target_schema: &str,
	from_config: &PgConfig,
	to_config: &PgConfig,
) -> std::io::Result<String> {
	let from_url = utils::url_encoded_connection_string(from_config);
	let to_url = utils::url_encoded_connection_string(to_config);

	let output = utils::temp_containers::podman_run(
		"votebase-dbdiff",
		// &format!("container:{}", db_container_name)
		&["--network", &podman_network.network_name],
		&["--with-privileges", "--schema", target_schema, &from_url, &to_url],
	).await?;

	// if !output.stderr.is_empty() {
	if !output.status.success() {
		let e = format!("dbdiff failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(std::io::Error::other(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct TempNetworkedPg {
	container_name: String,
	config: PgConfig,
	pg_pass: String,
	pg_user: String,
	pg_db: String,
	pg_port: u16,
}

pub async fn spawn_networked_postgres(
	db_name: String,
	podman_network: &utils::temp_containers::PodmanNetwork,
) -> std::io::Result<TempNetworkedPg> {
	let (container_name, config, pg_pass, pg_user, _pg_db, pg_port) = utils::temp_containers::generate_temp_config();

	// LEAK SAFETY we're only allowed to implicitly drop this process Child
	// because the podman network will be forcibly removed and therefore remove this podman process
	let _postgres_process = tokio::process::Command::new("podman")
		.args([
			"run",
			"--name", &container_name,
			"--network", &podman_network.network_name,
			"--env", &format!("POSTGRES_PASSWORD={pg_pass}"),
			"--env", &format!("POSTGRES_USER={pg_user}"),
			"--env", &format!("POSTGRES_DB={db_name}"),
			"--env", &format!("PGPORT={pg_port}"),
			"-p", &format!("{pg_port}:{pg_port}"),
			"--rm",
			"docker.io/library/postgres:latest",
		])
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn()?;

	utils::temp_containers::healthcheck_postgres(&config, 50, 100).await?;

	Ok(TempNetworkedPg {
		container_name, config, pg_pass, pg_user, pg_db: db_name, pg_port,
	})
}
