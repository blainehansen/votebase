use crate::{PgConfig};

pub async fn podman_compute_diff(
	db_container_name: &str,
	target_schema: &str,
	from_config: &PgConfig,
	to_config: &PgConfig,
) -> std::io::Result<String> {
	let from_url = crate::url_encoded_connection_string(from_config);
	let to_url = crate::url_encoded_connection_string(to_config);

	let output = temp_container_utils::podman_run(
		"votebase-dbdiff",
		&["--network", &format!("container:{}", db_container_name)],
		&["--with-privileges", "--schema", target_schema, &from_url, &to_url],
	).await?;

	// if !output.stderr.is_empty() {
	if !output.status.success() {
		let e = format!("dbdiff failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(std::io::Error::other(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn podman_pg_restore(
	db_container_name: &str,
	db_name: &str,
	db_user_name: &str,
	server_db_archive_path: &std::path::Path,
	// exclude_schema: Option<&str>,
) -> std::io::Result<()> {
	// pg_dump outputs to stdout if no --file argument is given, and pg_restore reads from stdin if no --file is given
	// if you go back to volumes: podman exec db_container_name pg_dump -d db_name -U db_user -f /whatever_volume_name/server_db_archive_path

	let mut command = tokio::process::Command::new("podman");
	command.arg("exec").arg(db_container_name)
		.arg("pg_restore")
		// --dbname=dbname
		.arg("-d").arg(db_name)
		// --username=username
		.arg("-U").arg(db_user_name)
		.arg("--format=custom")
		.arg("--schema-only")
		// --file=file
		// .arg("-f").arg(server_db_archive_path)
		.arg("--exit-on-error");

	// if let Some(exclude_schema) = exclude_schema {
	// 	// --exclude-schema=pattern
	// 	command.arg("-N").arg(exclude_schema);
	// }

	let mut child = command
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?;

	let mut server_db_archive = tokio::fs::File::open(server_db_archive_path).await?;
	let mut child_stdin = child.stdin.as_mut().ok_or_else(|| std::io::Error::other("unable to capture pg_restore stdin"))?;
	tokio::io::copy(&mut server_db_archive, &mut child_stdin).await?;

	let status = child.wait().await?;
	if status.success() { Ok(()) }
	else { Err(std::io::Error::other("pg_restore process failed")) }
}

pub async fn podman_pg_dump(
	db_container_name: &str,
	db_name: &str,
	db_user_name: &str,
	server_db_archive_path: &std::path::Path,
) -> std::io::Result<()> {
	let mut command = tokio::process::Command::new("podman");
	command.arg("exec").arg(db_container_name)
		.arg("pg_dump")
		// --dbname=dbname
		.arg("-d").arg(db_name)
		// --username=username
		.arg("-U").arg(db_user_name)
		.arg("--format=custom")
		.arg("--schema-only")
		// --file=file
		// .arg("-f").arg(server_db_archive_path)
		.arg("--exit-on-error");

	let mut child = command
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?;

	let mut server_db_archive = tokio::fs::File::open(server_db_archive_path).await?;
	let mut child_stdout = child.stdout.as_mut().ok_or_else(|| std::io::Error::other("unable to capture pg_dump stdout"))?;
	tokio::io::copy(&mut child_stdout, &mut server_db_archive).await?;

	let status = child.wait().await?;
	if status.success() { Ok(()) }
	else { Err(std::io::Error::other("pg_dump process failed")) }
}
