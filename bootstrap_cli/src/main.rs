use tokio_postgres as postgres;

type AnyError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).ok_or("first parameter should be a database url")?;

	let config = admin_connection_string.parse::<postgres::Config>()?;
	let (mut client, connection) = config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = connection.await { eprintln!("DB connection error: {}", e); } });

	let db_name = config.get_dbname().unwrap();
	let votebase_server_password = db_schema_utils::load_votebase_server_schema(&db_name, &client).await?;

	let bundled_ruleset_path = std::path::Path::new(args.get(1).ok_or("second parameter should be the bundled ruleset path")?);
	let bundled_ruleset: votebase_common::rulesets::BundledRuleset = serde_json::from_slice(tokio::fs::read(bundled_ruleset_path).await?.as_slice())?;

	let root_ruleset_name = args.get(2).map(String::as_str).unwrap_or("root");

	votebase_common::rulesets::validate_bundled_ruleset_top(
		None, root_ruleset_name, root_ruleset_name,
		None, &bundled_ruleset,
	).await?;

	votebase_common::rulesets::create_ruleset(
		&db_name,
		&mut client,
		None, root_ruleset_name,
		&bundled_ruleset,
	).await?;

	// TODO need to make sure any other logging that may happen during this cli invocation is redirected to stderr or something
	// proper logging done through logging crates probably goes to stderr regardless, so you just need to watch for errant println etc
	println!("{votebase_server_password}");

	Ok(())
}
