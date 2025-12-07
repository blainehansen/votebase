// https://www.postgresql.org/docs/current/app-pgrestore.html
use std::path::Path;
use votebase_common::postgres::{self, Config as PgConfig};

pub async fn cmd_generate_migration(ruleset_dir: &Path, server_db_archive_path: &Path) -> anyhow::Result<()> {
	let server_db_archive_path = server_db_archive_path.to_string_lossy();
	let db_schema_path = ruleset_dir.join("schema.sql");
	let db_schema = tokio::fs::read_to_string(db_schema_path).await?;

	let db_migration_str = temp_container_utils::with_temp_postgres_client(async |db_container_name, config, client| -> anyhow::Result<String> {
		// what we actually need to do here is use the client to create two diff databases
		let from_db_name = &format!("tempdb|from");
		let to_db_name = &format!("tempdb|to");
		client.batch_execute(&format!(r#"create database "{from_db_name}""#)).await?;
		client.batch_execute(&format!(r#"create database "{to_db_name}""#)).await?;

		let from_config = { let mut c = config.clone(); c.dbname(from_db_name); c };
		let to_config = { let mut c = config.clone(); c.dbname(to_db_name); c };

		// okay this isn't right
		// this is inherently a "bundle time" action! the db_migration is supposed to be a *real* migration, it's fully unnecessary until we're actually talking about slotting this ruleset into a real server
		// what this means is that we inherently also need to prepare the schema such that all the substitutions have happened!

		let (from_client, from_connection) = from_config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = from_connection.await { log::error!("DB connection error: {}", e); } });
		do_podman_pg_restore(&from_config, &server_db_archive_path).await?;
		from_client.batch_execute(&db_schema).await?;

		let (to_client, to_connection) = to_config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = to_connection.await { log::error!("DB connection error: {}", e); } });
		do_podman_pg_restore(&to_config, &server_db_archive_path).await?;
		to_client.batch_execute(&db_schema).await?;



		let pgschema = "my_theoretical_ruleset";

		// into a "from" load just the server_db_archive_path
		// into a "to" load the server_db_archive_path and the db_schema
		// diff the two by calling a podman uv or whatever to call migra or whatever thing you choose
		// then you have a diff and you're done

		Ok(compute_diff(pgschema, &db_container_name, &from_config, &to_config).await?)
	}).await??;

	tokio::fs::write(ruleset_dir.join("db_migration.sql"), db_migration_str).await?;

	Ok(())
}

async fn do_podman_pg_restore(db_container_name: &str, server_db_archive_path: &str) -> anyhow::Result<std::process::Output> {
	let connection_string = votebase_common::url_encoded_connection_string(&config);

	let output = tokio::process::Command::new("pg_restore")
		.arg("--dbname")
		.arg(connection_string)
		.arg("--format=custom")
		.arg("--schema-only")
		.arg(format!("--file={server_db_archive_path}"))
		.arg("--exit-on-error")
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;

	Ok(output)
}


pub async fn compute_diff(
	pgschema: &str,
	db_container_name: &str,
	from_config: &PgConfig,
	to_config: &PgConfig,
) -> anyhow::Result<String> {
	// #[cfg(debug_assertions)]
	// let mut command = {
	// 	let mut command = tokio::process::Command::new("uv");
	// 	command.args("tool run -p 3.11 --with psycopg2-binary --with setuptools migra".split_whitespace());
	// 	command
	// };
	// #[cfg(not(debug_assertions))]
	// let mut command = tokio::process::Command::new("migra");

	let from_url = votebase_common::url_encoded_connection_string(from_config);
	let to_url = votebase_common::url_encoded_connection_string(to_config);

	let output = temp_container_utils::podman_run(
		"votebase-dbdiff",
		&["--network", &format!("container:{}", db_container_name)],
		&["--with-privileges", "--schema", pgschema, &from_url, &to_url],
	).await?;

	// if !output.stderr.is_empty() {
	if !output.status.success() {
		return Err(anyhow::anyhow!("dbdiff failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr)));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
