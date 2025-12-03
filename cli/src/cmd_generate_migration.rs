// https://www.postgresql.org/docs/current/app-pgrestore.html
use std::path::Path;

pub async fn cmd_generate_migration(ruleset_dir: &Path, server_db_archive: &Path) -> anyhow::Result<()> {
	let server_db_archive = server_db_archive.to_string_lossy();
	let db_schema_path = ruleset_dir.join("schema.sql");

	let db_migration_str = temp_container_utils::with_temp_postgres_client(async |config, mut client| -> anyhow::Result<String> {
		// what we actually need to do here is use the client to create two diff databases
		// into a "from" load just the server_db_archive
		// into a "to" load the server_db_archive and the db_schema
		// diff the two by calling a podman uv or whatever to call migra or whatever thing you choose
		// then you have a diff and you're done

		let connection_string = votebase_common::url_encoded_connection_string(&config);

		tokio::process::Command::new("pg_restore")
			.arg("--dbname")
			.arg(connection_string)
			.arg("--format=custom")
			.arg("--schema-only")
			.arg(format!("--file={server_db_archive}"))
			.arg("--exit-on-error")
			.stderr(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.spawn()?.wait_with_output().await?;

		// Ok(db_migration_str)
		unimplemented!()
	}).await??;

	tokio::fs::write(ruleset_dir.join("db_migration.sql"), db_migration_str).await?;

	Ok(())
}
