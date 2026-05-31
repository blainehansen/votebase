use crate::{PgClient, PgConfig};

pub async fn podman_compute_diff(
	podman_network: &utils::temp_containers::PodmanNetwork,
	target_schema: &str,
	from_config: &PgConfig,
	to_config: &PgConfig,
) -> std::io::Result<String> {
	let from_url = utils::url_encoded_connection_string(from_config);
	let to_url = utils::url_encoded_connection_string(to_config);

	// podman network create testing_testing_net

	// podman run --name temp_postgres_a --network testing_testing_net --rm -e POSTGRES_PASSWORD=dev_admin_password -e POSTGRES_USER=dev_admin_user -e POSTGRES_DB=dev_db -p 127.0.0.1::5432 docker.io/library/postgres:18-alpine

	// podman port temp_postgres_a 5432/tcp


	// podman run --name temp_postgres_b --network testing_testing_net --rm -e POSTGRES_PASSWORD=dev_admin_password -e POSTGRES_USER=dev_admin_user -e POSTGRES_DB=dev_db -p 127.0.0.1::5432 docker.io/library/postgres:18-alpine

	// podman port temp_postgres_b 5432/tcp

	// podman run --rm --network testing_testing_net votebase-dbdiff \
	// 	--with-privileges \
	// 	"postgresql://dev_admin_user:dev_admin_password@temp_postgres_a:5432/dev_db" \
	// 	"postgresql://dev_admin_user:dev_admin_password@temp_postgres_b:5432/dev_db"


	let output = utils::temp_containers::podman_run(
		"votebase-dbdiff",
		// &format!("container:{}", db_container_name)
		&["--network", &podman_network.network_name],
		&["--with-privileges", "--schema", target_schema, &dbg!(from_url), &dbg!(to_url)],
	).await?;

	// if !output.stderr.is_empty() {
	if !output.status.success() {
		let e = format!("dbdiff failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(std::io::Error::other(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub struct TempNetworkedPg {
	pub server_config: PgConfig,
	pub server_client: PgClient,
	#[allow(dead_code)]
	postgres_process: utils::tokio_graceful_spawn::GracefulChild,
}

pub async fn spawn_networked_votebase_postgres(
	podman_network: &utils::temp_containers::PodmanNetwork,
) -> std::io::Result<TempNetworkedPg> {
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = utils::temp_containers::generate_temp_config();

	use utils::tokio_graceful_spawn::GracefulSpawn;
	let postgres_process = tokio::process::Command::new("podman")
		.args([
			"run",
			"--name", &container_name,
			"--network", &podman_network.network_name,
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

	utils::temp_containers::healthcheck_postgres(&config, 50, 100).await?;

	let admin_client = crate::pg_con(&config).await.map_err(std::io::Error::other)?;
	let server_password = db_schema_utils::load_votebase_server_schema(&pg_db, &admin_client).await.map_err(std::io::Error::other)?;
	drop(admin_client);

	let mut server_config = config;
	server_config.user("votebase_server");
	server_config.password(server_password);

	let server_client = crate::pg_con(&server_config).await.map_err(std::io::Error::other)?;

	Ok(TempNetworkedPg { server_config, server_client, postgres_process })
}
