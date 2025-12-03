mod cmd_init;
pub use cmd_init::cmd_init;

mod cmd_dev_check;
pub use cmd_dev_check::{cmd_dev, cmd_check};

// TODO need to test situations where rulesets are incorrect, either for structural but especially permissions reasons

// bad_ts the ruleset file itself is malformed. honestly this probably should be mostly done in a series of typescript tests etc

// both of these are very important, they're the places where we'll do things like check that the permissions system is working correctly, and that the schema naming and relationships make sense
// but, this should probably be tested at the level of run_sql_checking_and_generation rather than here
// check:
// - we can't really tell the difference between an action and a query.... hmmm. the cli should probably have a "test" command that runs the real ruleset fns on randomly generated input, which we'll be able to do if they declare an input schema!
// - the schema isn't trying to "reach outside" of itself to modify other schemas or set its own search path etc. it isn't trying to modify the catalog, or create objects it isn't allowed to create


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
