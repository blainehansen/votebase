use tokio_postgres as postgres;

type AnyError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("first parameter should be a database url");
	let config = admin_connection_string.parse::<postgres::Config>()?;
	let (mut client, connection) = config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });

	let db_name = config.get_dbname().unwrap().to_owned();
	let votebase_server_password = db_schema_utils::load_votebase_server_schema(&db_name, &client).await?;

	votebase_common::rulesets::create_ruleset(
		&db_name,
		&mut client,
		None, "root",
		&votebase_common::rulesets::BundledRuleset {
			ts_code: include_str!("../../rulesets/accept-any/ruleset.ts"),
			db_schema: "".to_string(),
			db_migration: "".to_string(),
			fns: vec![],
		},
	).await?;

	println!("{votebase_server_password}");

	Ok(())
}
