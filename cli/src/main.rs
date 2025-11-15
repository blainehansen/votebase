use votebase_common::{postgres, deadpool, queries};
use tokio::io::AsyncWriteExt;

type AnyError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: VotebaseCliArgs = argh::from_env();

	let config = args.db_url.parse::<postgres::Config>()?;
	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(1).build()?;
	let client = pool.get().await?;

	let ruleset_dir = args.ruleset_dir;
	let queries_dir = ruleset_dir.join("queries");
	match args.subcommand {
		SubCommand::Dev(_) => {
			let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
			let temp_dbname = format!("votebase_temp_db_{now}");
			client.execute(&format!(r#"create database "{temp_dbname}""#), &[]).await?;

			let generated = {
				let mut temp_config = config.clone();
				temp_config.dbname(&temp_dbname);
				let temp_pool = deadpool::Pool::builder(deadpool::Manager::new(temp_config, postgres::NoTls)).max_size(1).build()?;
				let mut temp_client = temp_pool.get().await?;

				let schema_sql = format!(include_str!("../../bootstrap_cli/schema.sql"), db_database=temp_dbname, votebase_server_password="votebase_server_password");
				temp_client.batch_execute(&schema_sql).await?;
				temp_client.batch_execute(&format!(r#"create schema public; alter database "{temp_dbname}" reset search_path;"#)).await?;

				votebase_common::runtime::create_ruleset(
					&config, &mut temp_client,
					None, "root",
					&vec!["__insert_initial".to_string()], &vec![],
					include_str!("../../rulesets/accept-any/ruleset.ts"), "",
				).await?;

				let db_schema = tokio::fs::read_to_string(ruleset_dir.join("schema.sql")).await?;
				temp_client.batch_execute(&db_schema).await?;

				let generated_fields = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &temp_client).await?;
				format!("import 'votebase'\nexport default {{\n{generated_fields}\n}}")
			};
			client.batch_execute(&format!(r#"drop database if exists "{temp_dbname}""#)).await?;

			let mut file = tokio::fs::OpenOptions::new().write(true).create(true)
				.open(format!("{}.ts", queries_dir.to_string_lossy())).await?;

			file.write_all(generated.as_bytes()).await?;
		},

		SubCommand::Check(_) => {

		},

		SubCommand::Bundle(Bundle { migration_file }) => {
			let generated_fields = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &client).await?;

			let existing_code = tokio::fs::read_to_string(ruleset_dir.join("ruleset.ts")).await?;
			// TODO	have to to figure out what the existing code imports it as and strip it out
			let code = format!("const queries = {{\n{generated_fields}\n}}\n{existing_code}");
			let db_schema = tokio::fs::read_to_string(ruleset_dir.join("schema.sql")).await?;
			let db_migration = tokio::fs::read_to_string(migration_file).await?;
			// TODO check the db_migration against the real current schema (where do we get that from???) and the stated final schema

			let bundled_ruleset = serde_json::to_string(&BundledRuleset { code, db_schema, db_migration })?;
			let bundle_file = ruleset_dir.join("ruleset.json");
			let mut file = tokio::fs::OpenOptions::new().write(true).create(true).truncate(true)
				.open(bundle_file).await?;
			file.write_all(bundled_ruleset.as_bytes()).await?;
		},
	}

	Ok(())
}

#[derive(Debug, serde::Serialize)]
struct BundledRuleset {
	code: String,
	db_schema: String,
	db_migration: String,
}

#[derive(argh::FromArgs, Debug)]
/// votebase ruleset development cli
struct VotebaseCliArgs {
	/// database URL
	#[argh(option)]
	db_url: String,

	/// ruleset directory structured the following way:
	/// - `queries` directory containing all the sql operations for the ruleset, one callable sql group per file
	/// - `schema.sql` file containing the desired *final* sql schema for the ruleset, as if it were being created from an empty database
	/// - `ruleset.ts` the actual ruleset code that defines the Actions and Views
	#[argh(option)]
	ruleset_dir: std::path::PathBuf,

	#[argh(subcommand)]
	subcommand: SubCommand,
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand)]
enum SubCommand {
	Dev(Dev),
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
	/// the sql file meant to successfully migrate the particular ruleset to the final state specified in this ruleset's `schema.sql`
	#[argh(option)]
	migration_file: std::path::PathBuf,
}

