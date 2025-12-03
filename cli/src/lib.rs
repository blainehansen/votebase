mod cmd_init;
pub use cmd_init::cmd_init;


use std::{collections::HashMap, path::Path};

// - in a temp podman postgres
//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
//   - generate the queries and write them into the file
pub async fn cmd_dev(ruleset_dir: &Path) -> anyhow::Result<()> {
	run_sql_checking_and_generation(&ruleset_dir, true).await
}

// - in a temp podman postgres
//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
//   - generate the queries and write them into the file
// - using a temp podman typescript, runs a typecheck with the votebase tsconfig
pub async fn cmd_check(ruleset_dir: &Path) -> anyhow::Result<()> {
	run_sql_checking_and_generation(&ruleset_dir, true).await?;
	run_votebase_tsc(&ruleset_dir).await
}
pub async fn cmd_fetch_server_schema() -> anyhow::Result<()> {
	Ok(())
	//
}
pub async fn cmd_generate_migration() -> anyhow::Result<()> {
	Ok(())
	//
}

// - fetch the pg archive and the full Ruleset tree from the real server with whatever caching rules (???) (hash the schema, and when the dev tools request the schema they can specify which one they already have including null, and server tells them they're good if nothing's changed)
// - in a temp podman postgres
//   - restore the pg archive to a "current" db
//   - fulfill `schema.sql` using the actual provided values in `vars`, then write it to an "intended" db, then get a diff from "current" to "intended", using the `schema` parameter to narrow to only this ruleset. then apply that diff as the migration on top of the archive, to be used for real typechecking. can do shenanigans with diffing the archive against nothing with a schema narrowing to get the "current" standalone schema, perhaps
//   - generate the queries and write them into the file
// - using a temp podman typescript, run a typecheck with the votebase tsconfig
// - analyze the ruleset by executing it, and use the migration generated above to create the bundle
pub async fn cmd_bundle() -> anyhow::Result<()> {
	Ok(())
	// let generated_fields = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &client).await?;

	// let existing_code = tokio::fs::read_to_string(ruleset_dir.join("ruleset.ts")).await?;
	// // TODO	have to to figure out what the existing code imports it as and strip it out
	// let code = format!("const queries = {{\n{generated_fields}\n}}\n{existing_code}");
	// let db_schema = tokio::fs::read_to_string(ruleset_dir.join("schema.sql")).await?;
	// let db_migration = tokio::fs::read_to_string(migration_file).await?;
	// // TODO check the db_migration against the real current schema (where do we get that from???) and the stated final schema

	// let bundled_ruleset = serde_json::to_string(&BundledRuleset { code, db_schema, db_migration })?;
	// let bundle_file = ruleset_dir.join("ruleset.json");

	// use tokio::io::AsyncWriteExt;
	// let mut file = tokio::fs::OpenOptions::new().write(true).create(true).truncate(true)
	// 	.open(bundle_file).await?;
	// file.write_all(bundled_ruleset.as_bytes()).await?;
}



async fn run_sql_checking_and_generation(ruleset_dir: &Path, do_generation: bool) -> anyhow::Result<()> {
	let queries_dir = ruleset_dir.join("queries");
	let schema_file = ruleset_dir.join("schema.sql");

	let generated = temp_container_utils::with_temp_postgres_client(async |db_config, mut client| -> anyhow::Result<String> {
		let db_name = db_config.get_dbname().unwrap().to_string();
		println!("loading votebase schema");
		db_schema_utils::load_votebase_server_schema(db_name, &mut client).await?;

		println!("loading ruleset schema");
		let ruleset_db_schema = if tokio::fs::try_exists(&schema_file).await.unwrap_or(false) {
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

async fn run_votebase_tsc(ruleset_dir: &Path) -> anyhow::Result<()> {
	let ruleset_dir = std::env::current_dir()?.join(ruleset_dir);
	let workspace_arg = format!("{}:/workspace/ruleset", ruleset_dir.to_string_lossy());
	let output = temp_container_utils::run_workspace_podman_cmd("votebase-tsc", workspace_arg, &["--noEmit"]).await?;

	if !output.status.success() {
		// let all_output = output.stdout.extend(output.stderr);
		let err = String::from_utf8_lossy(&output.stdout);
		Err(anyhow::anyhow!("typescript errors when checking with the votebase tsconfig:\n\n{err}"))
	}
	else { Ok(()) }
}



struct BundleInfo {
	/// the path of the ruleset that this bundled ruleset is intended to replace, if it's a replacement
	ruleset_path: Option<String>,

	/// the url of the server the final bundled ruleset is intended for
	server_url: url::Url,

	/// an optional mapping from the abstract "var" name in this Ruleset to the fully qualified name actually intended
	db_uses: HashMap<String, String>,

	/// the migration intended to actually be run to reach the state of db_schema. used in `generate_migration` to determine the destination to write to, overwriting the existing migration
	db_migration_file: std::path::PathBuf,

	// /// a list of fully qualified Views that this Ruleset relies on
	// view_uses: Vec<String>
	// /// a list of fully qualified Actions that this Ruleset relies on
	// action_uses: Vec<String>

	// /// a mapping from names to KeepOrReplace of pairings of further ruleset directories and vars. all others not mentioned here are deleted
	// static_children: HashMap<String, KeepOrReplace<(String, BundleInfo)>>,
	// /// a predicate that determines what dynamic children to keep, and all others will be recursively deleted
	// dynamic_children_keep_rule: String,

	// /// a list of static recurring actions of this Ruleset. all others not mentioned here are deleted
	// static_recurring_events: Vec<StaticRecurringEvent>,
	// /// a predicate that determines what dynamic recurring children to keep, and all others will be recursively deleted
	// dynamic_recurring_event_keep_rule: String,
	// /// a predicate that determines what dynamic standalone children to keep, and all others will be recursively deleted
	// dynamic_standalone_event_keep_rule: String,
}

// enum KeepOrReplace<R> {
// 	Keep,
// 	Replace(R),
// }
