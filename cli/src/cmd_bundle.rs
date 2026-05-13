// - fetch the pg archive and the full Ruleset tree from the real server with whatever caching rules (???) (hash the schema, and when the dev tools request the schema they can specify which one they already have including null, and server tells them they're good if nothing's changed)
// - in a temp podman postgres
//   - restore the pg archive to a "current" db
//   - fulfill `schema.sql` using the actual provided values in `vars`, then write it to an "intended" db, then get a diff from "current" to "intended", using the `schema` parameter to narrow to only this ruleset. then apply that diff as the migration on top of the archive, to be used for real typechecking. can do shenanigans with diffing the archive against nothing with a schema narrowing to get the "current" standalone schema, perhaps
//   - generate the queries and write them into the file
// - using a temp podman typescript, run a typecheck with the votebase tsconfig
// - analyze the ruleset by executing it, and use the migration generated above to create the bundle
pub async fn cmd_bundle(ruleset_dir: &std::path::Path, bundle_path: &std::path::Path) -> anyhow::Result<()> {
	// crate::cmd_check(ruleset_dir).await?;

	// let queries_dir = ruleset_dir.join("queries");
	let ts_code_file = ruleset_dir.join("ruleset.ts");
	let schema_file = ruleset_dir.join("schema.sql");
	let migration_file = ruleset_dir.join("migration.sql");

	println!("before join");
	let (existing_code, db_schema, db_migration) = tokio::try_join!(
		tokio::fs::read_to_string(&ts_code_file),
		read_file_or_none(&schema_file),
		read_file_or_none(&migration_file),
	)?;
	println!("after join");

	// let generated_fields = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &client).await?;
	// TODO	have to to figure out what the existing code imports it as and strip it out
	// let ts_code = format!("const queries = {{\n{generated_fields}\n}}\n{existing_code}");
	let ts_code = existing_code;
	// TODO check the db_migration against the real current schema (where do we get that from???) and the stated final schema

	let (db_schema, db_migration) = match (db_schema, db_migration) {
		(Some(db_schema), Some(db_migration)) => (db_schema, db_migration),
		(Some(db_schema), None) => (db_schema.clone(), db_schema),
		(None, None) => ("".to_string(), "".to_string()),
		(None, Some(_)) => return Err(anyhow::anyhow!("it doesn't make any sense to have a migration.sql but no schema.sql")),
	};

	let fns = votebase_common::rulesets::determine_fns(&ts_code).await?;
	let bundled_ruleset = serde_json::to_string(&votebase_common::rulesets::BundledRuleset {
		ts_code, db_schema, db_migration, fns,
	})?;
	let bundle_file = std::env::current_dir()?.join(bundle_path);

	use tokio::io::AsyncWriteExt;
	let mut file = tokio::fs::OpenOptions::new().write(true).create(true).truncate(true)
		.open(bundle_file).await?;
	file.write_all(bundled_ruleset.as_bytes()).await?;

	Ok(())
}

async fn read_file_or_none(path: &std::path::Path) -> Result<Option<String>, std::io::Error> {
	match tokio::fs::read_to_string(path).await {
		Ok(content) => Ok(Some(content)),
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(e) => Err(e),
	}
}

// struct BundleInfo {
// 	/// the path of the ruleset that this bundled ruleset is intended to replace, if it's a replacement
// 	ruleset_path: Option<String>,

// 	/// the url of the server the final bundled ruleset is intended for
// 	server_url: url::Url,

// 	/// an optional mapping from the abstract "var" name in this Ruleset to the fully qualified name actually intended
// 	db_uses: std::collections::HashMap<String, String>,

// 	/// the migration intended to actually be run to reach the state of db_schema. used in `generate_migration` to determine the destination to write to, overwriting the existing migration
// 	db_migration_file: std::path::PathBuf,

// 	// /// a list of fully qualified Views that this Ruleset relies on
// 	// view_uses: Vec<String>
// 	// /// a list of fully qualified Actions that this Ruleset relies on
// 	// action_uses: Vec<String>

// 	// /// a mapping from names to KeepOrReplace of pairings of further ruleset directories and vars. all others not mentioned here are deleted
// 	// static_children: HashMap<String, KeepOrReplace<(String, BundleInfo)>>,
// 	// /// a predicate that determines what dynamic children to keep, and all others will be recursively deleted
// 	// dynamic_children_keep_rule: String,

// 	// /// a list of static recurring actions of this Ruleset. all others not mentioned here are deleted
// 	// static_recurring_events: Vec<StaticRecurringEvent>,
// 	// /// a predicate that determines what dynamic recurring children to keep, and all others will be recursively deleted
// 	// dynamic_recurring_event_keep_rule: String,
// 	// /// a predicate that determines what dynamic standalone children to keep, and all others will be recursively deleted
// 	// dynamic_standalone_event_keep_rule: String,
// }

// enum KeepOrReplace<R> {
// 	Keep,
// 	Replace(R),
// }
