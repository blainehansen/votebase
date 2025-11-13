use tokio_postgres as postgres;
use deadpool_postgres as deadpool;

type AnyError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("first parameter should be a database url");
	let config = admin_connection_string.parse::<postgres::Config>()?;
	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(5).build()?;

	let mut client = pool.get().await?;

	let db_name = config.get_dbname().unwrap().to_owned();
	let votebase_server_password = votebase_common::load_votebase_server_schema(db_name, &mut client).await?;

	votebase_common::runtime::create_ruleset(
		&config, &mut client,
		None, "root",
		&vec!["__insert_initial".to_string()], &vec![],
		include_str!("../../rulesets/accept-any/ruleset.ts"), "",
	).await?;

	println!("{votebase_server_password}");

	Ok(())
}
