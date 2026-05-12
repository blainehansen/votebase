// TODO https://rust-cli.github.io/book/tutorial/testing.html

use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args: VotebaseCliArgs = argh::from_env();

	let ruleset_dir = args.ruleset_dir;
	match args.subcommand {
		SubCommand::Init(_) => {
			tokio::task::spawn_blocking(move || {
				votebase_cli::cmd_init(&ruleset_dir)
			}).await??;
		},
		// SubCommand::Dev(_) => {
		// 	votebase_cli::cmd_dev(&ruleset_dir).await?;
		// },
		// SubCommand::Check(_) => {
		// 	votebase_cli::cmd_check(&ruleset_dir).await?;
		// },
		// SubCommand::CreateBundleInfo(_) => {
		// 	votebase_cli::cmd_create_bundle_info()?;
		// },
		// SubCommand::FetchServerSchema(_) => {
		// 	votebase_cli::cmd_fetch_server_schema().await?;
		// },
		SubCommand::GenerateMigration(_) => {
			unimplemented!();
			// votebase_cli::cmd_generate_migration(&ruleset_dir, &TODO).await?;
		},
		SubCommand::CheckMigration(_) => {
			unimplemented!();
		},
		SubCommand::Bundle(Bundle { bundle_path }) => {
			votebase_cli::cmd_bundle(&ruleset_dir, &bundle_path).await?;
		},
	}

	Ok(())
}


#[derive(argh::FromArgs, Debug)]
/// votebase ruleset development cli
struct VotebaseCliArgs {
	/// the path of the ruleset you are targeting
	#[argh(positional)]
	ruleset_dir: PathBuf,

	#[argh(subcommand)]
	subcommand: SubCommand,
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand)]
enum SubCommand {
	Init(Init),
	// Dev(Dev),
	// Check(Check),
	// CreateBundleInfo(CreateBundleInfo),
	// FetchServerSchema(FetchServerSchema),
	GenerateMigration(GenerateMigration),
	CheckMigration(CheckMigration),
	Bundle(Bundle),
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "init")]
/// Sets up a directory with the skeleton of a new Ruleset.
struct Init {}

// #[derive(argh::FromArgs, Debug)]
// #[argh(subcommand, name = "dev")]
// /// prepare all ruleset queries and place them in the dev typescript file next to the queries directory (e.g. for `./queries/` dir `./queries.ts`)
// struct Dev {}

// #[derive(argh::FromArgs, Debug)]
// #[argh(subcommand, name = "check")]
// /// prepare all ruleset queries and place them in the dev typescript file next to the queries directory (e.g. for `./queries/` dir `./queries.ts`), and run the Typescript check
// struct Check {}

// #[derive(argh::FromArgs, Debug)]
// #[argh(subcommand, name = "create_bundle_info")]
// /// TODO
// struct CreateBundleInfo {
// 	// /// TODO
// 	// #[argh(option)]
// 	// db_migration_file: url::Url,
// }

// #[derive(argh::FromArgs, Debug)]
// #[argh(subcommand, name = "fetch_server_schema")]
// /// TODO
// struct FetchServerSchema {
// 	// /// TODO
// 	// #[argh(option)]
// 	// server_url: url::Url,
// }

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "generate_migration")]
/// Create a first draft migration intended to go from the schema specified in the current server information to the one your ruleset specifies. You might want to modify the migration.
struct GenerateMigration {}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "check_migration")]
/// Check that the final schema and the db_migration align. This check is also performed when you call `bundle` and `check`, so it's intended for situations where you want to just run this check quickly. Prints out the sql necessary to go from db_migration to db_schema if there are discrepancies.
struct CheckMigration {}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "bundle")]
/// Bundle the ruleset as a json object ready to be proposed in through the `op_propose_self_replacement` runtime function.
struct Bundle {
	#[argh(positional)]
	bundle_path: PathBuf,

	// #[argh(option)]
	// vars_file: PathBuf,
}
