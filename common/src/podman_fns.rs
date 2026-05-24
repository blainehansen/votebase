use crate::{PgClient, PgConfig};

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

pub struct TempNetworkedPg {
	pub container_name: String,
	pub server_config: PgConfig,
	pub server_client: PgClient,
	postgres_process: utils::tokio_graceful_spawn::GracefulChild,
}

pub async fn spawn_networked_votebase_postgres(
	db_name: String,
	podman_network: &utils::temp_containers::PodmanNetwork,
) -> std::io::Result<TempNetworkedPg> {
	let (container_name, config, pg_pass, pg_user, _pg_db, pg_port) = utils::temp_containers::generate_temp_config();

	use utils::tokio_graceful_spawn::GracefulSpawn;
	let postgres_process = tokio::process::Command::new("podman")
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
		.graceful_spawn()?;

	utils::temp_containers::healthcheck_postgres(&config, 50, 100).await?;

	let admin_client = crate::pg_con(&config).await.map_err(std::io::Error::other)?;
	let server_password = db_schema_utils::load_votebase_server_schema(&db_name, &admin_client).await.map_err(std::io::Error::other)?;
	drop(admin_client);

	let mut server_config = config;
	server_config.user(format!("votebase_server_{db_name}"));
	server_config.password(server_password);

	let server_client = crate::pg_con(&server_config).await.map_err(std::io::Error::other)?;

	Ok(TempNetworkedPg { container_name, server_config, server_client, postgres_process })
}
