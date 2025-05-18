use votebase_queries::{tokio_postgres as postgres, deadpool_postgres as deadpool};

type AnyError = Box<dyn std::error::Error>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("first parameter should be a database url");
	let config = admin_connection_string.parse::<postgres::Config>()?;
	let pool = deadpool::Pool::builder(deadpool::Manager::new(config.clone(), postgres::NoTls)).max_size(5).build()?;

	let mut client = pool.get().await?;

	#[cfg(debug_assertions)]
	let votebase_server_password = "votebase_server_dev_pass".to_string();
	#[cfg(not(debug_assertions))]
	let votebase_server_password = {
		use base64::Engine;
		use rand::{Rng, SeedableRng};
		let mut random_bytes = [0u8; 526];
		let mut rng = rand::rngs::StdRng::from_os_rng();
		rng.fill(&mut random_bytes);
		base64::prelude::BASE64_STANDARD.encode(random_bytes)
	};

	let db_database = config.get_dbname().unwrap().to_owned();
	let schema_sql = format!(include_str!("../schema.sql"), db_database=db_database, votebase_server_password=votebase_server_password);
	client.batch_execute(&schema_sql).await?;

	votebase_common::runtime::create_ruleset(
		&config, &mut client,
		None, "root",
		&vec!["__insert_initial".to_string()], &vec![],
		include_str!("../../rulesets/accept-any/ruleset.ts"), "",
	).await?;

	println!("{votebase_server_password}");

	Ok(())
}
