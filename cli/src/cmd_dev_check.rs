use std::path::Path;

pub async fn cmd_dev(ruleset_dir: &Path) -> anyhow::Result<()> {
	run_sql_checking_and_generation(&ruleset_dir, true).await
}

// - in a temp podman postgres
//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
//   - generate the queries and write them into the file
// - using a temp podman typescript, runs a typecheck with the votebase tsconfig
pub async fn cmd_check(ruleset_dir: &Path) -> anyhow::Result<()> {
	cmd_dev(ruleset_dir).await?;
	let full_ruleset_dir = std::env::current_dir()?.join(ruleset_dir);
	votebase_common::runtime::rulesets::podman_votebase_tsc(full_ruleset_dir).await?;

	Ok(())
}


async fn run_sql_checking_and_generation(ruleset_dir: &Path, do_generation: bool) -> anyhow::Result<()> {
	let queries_dir = ruleset_dir.join("queries");
	let schema_file = ruleset_dir.join("schema.sql");

	let generated = utils::temp_containers::with_temp_postgres_client(async |_, db_config, mut client| -> anyhow::Result<String> {
		let db_name = db_config.get_dbname().unwrap();
		println!("loading votebase schema");
		db_schema_utils::load_votebase_server_schema(db_name, &mut client).await?;

		println!("loading ruleset schema");
		let ruleset_db_schema =
			if tokio::fs::try_exists(&schema_file).await.unwrap_or(false) {
				tokio::fs::read_to_string(&schema_file).await?
			}
			else {
				println!("no schema.sql file found, assuming no special schema");
				"".to_string()
			};
		let migrator_client = votebase_common::runtime::rulesets::create_ruleset(
			&db_config, &mut client,
			None, "root",
			&vec![], &vec![],
			"", &ruleset_db_schema,
		).await?;

		println!("generating");
		// TODO use the migrator role?
		let generated_fields = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &migrator_client).await?;
		Ok(format!("export default {{\n{generated_fields}\n}}\n"))
	}).await??;

	if do_generation {
		println!("opening");
		use tokio::io::AsyncWriteExt;
		let mut file = tokio::fs::OpenOptions::new().write(true).create(true).truncate(true)
			.open(format!("{}.ts", queries_dir.to_string_lossy())).await?;

		println!("writing");
		file.write_all(generated.as_bytes()).await?;
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use assert_fs::prelude::*;
	use predicates::prelude::*;

	#[test]
	fn test_cmd_check_test_rulesets() {
		use predicate::path;

		let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

		for entry in std::fs::read_dir("../test-rulesets").unwrap() {
			let entry = entry.unwrap().path();
			if !entry.is_dir() { continue; }

			let temp_dir = assert_fs::TempDir::new().unwrap();
			temp_dir.copy_from(&entry, &["**", "!queries.ts"]).unwrap();
			temp_dir.child("queries.ts").assert(path::missing());
			let temp_dir_path = temp_dir.path();

			runtime.block_on(cmd_check(temp_dir_path)).unwrap();

			let expected_queries_path = entry.with_extension("expected.queries.ts");
			// print!("{}", tokio::fs::read_to_string(entry.with_extension("expected.queries.ts")).await.unwrap());
			temp_dir.child("queries.ts").assert(path::eq_file(&expected_queries_path));
		}
	}
}
