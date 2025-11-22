// TODO https://rust-cli.github.io/book/tutorial/testing.html


// use votebase_common::{postgres, deadpool, queries};

use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args: VotebaseCliArgs = argh::from_env();

	// let config = args.db_url.parse::<postgres::Config>()?;
	// let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(1).build()?;
	// let client = pool.get().await?;

	let ruleset_dir = args.ruleset_dir;
	match args.subcommand {
		// - in a temp podman postgres
		//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
		//   - generate the queries and write them into the file
		SubCommand::Dev(_) => {
			run_sql_checking_and_generation(&ruleset_dir, true).await?;
		},

		// - in a temp podman postgres
		//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
		//   - generate the queries and write them into the file
		// - using a temp podman typescript, runs a typecheck with the votebase tsconfig
		SubCommand::Check(_) => {
			run_sql_checking_and_generation(&ruleset_dir, true).await?;
			run_votebase_tsc(&ruleset_dir).await?;
		},

		// - fetch the pg archive and the full Ruleset tree from the real server with whatever caching rules (???) (hash the schema, and when the dev tools request the schema they can specify which one they already have including null, and server tells them they're good if nothing's changed)
		// - in a temp podman postgres
		//   - restore the pg archive to a "current" db
		//   - fulfill `schema.sql` using the actual provided values in `vars`, then write it to an "intended" db, then get a diff from "current" to "intended", using the `schema` parameter to narrow to only this ruleset. then apply that diff as the migration on top of the archive, to be used for real typechecking. can do shenanigans with diffing the archive against nothing with a schema narrowing to get the "current" standalone schema, perhaps
		//   - generate the queries and write them into the file
		// - using a temp podman typescript, run a typecheck with the votebase tsconfig
		// - analyze the ruleset by executing it, and use the migration generated above to create the bundle
		SubCommand::Bundle(Bundle { /*migration_file*/ }) => {
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
		},
	}

	Ok(())
}

async fn run_sql_checking_and_generation(ruleset_dir: &PathBuf, do_generation: bool) -> anyhow::Result<()> {
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
		Ok(format!("export default {{\n{generated_fields}\n}}"))
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

async fn run_votebase_tsc(ruleset_dir: &PathBuf) -> anyhow::Result<()> {
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






// #[derive(Debug, serde::Serialize)]
// struct BundledRuleset {
// 	code: String,
// 	db_schema: String,
// 	db_migration: String,
// }

#[derive(argh::FromArgs, Debug)]
/// votebase ruleset development cli
struct VotebaseCliArgs {
	// /// database URL
	// #[argh(option)]
	// db_url: String,

	/// ruleset directory structured the following way:
	/// - `queries` directory containing all the sql operations for the ruleset, one callable sql group per file
	/// - `schema.sql` file containing the desired *final* sql schema for the ruleset, as if it were being created from an empty database
	/// - `ruleset.ts` the actual ruleset code that defines the Actions and Views
	#[argh(option)]
	ruleset_dir: PathBuf,

	#[argh(subcommand)]
	subcommand: SubCommand,
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand)]
enum SubCommand {
	Dev(Dev),
	Check(Check),
	Bundle(Bundle),
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "dev")]
/// prepare all ruleset queries and place them in the dev typescript file next to the queries directory (e.g. for `./queries/` dir `./queries.ts`)
struct Dev {}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "check")]
/// prepare all ruleset queries and place them in the dev typescript file next to the queries directory (e.g. for `./queries/` dir `./queries.ts`), and run the Typescript check
struct Check {}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "bundle")]
/// bundle the ruleset as a json object ready to be proposed in through the `op_propose_self_replacement` runtime function
struct Bundle {
	// /// the sql file meant to successfully migrate the particular ruleset to the final state specified in this ruleset's `schema.sql`
	// #[argh(option)]
	// migration_file: PathBuf,
}

