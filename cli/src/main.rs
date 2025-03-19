#[tokio::main]
async fn main() -> std::io::Result<()> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("first and only parameter should be a database url");

	let db_url: sqlx::postgres::PgConnectOptions = admin_connection_string.parse().expect("invalid db url");
	let db_database = db_url.get_database().unwrap().to_owned();
	let pool = sqlx::PgPool::connect_with(db_url).await.expect("unable to connect");

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

	sqlx::raw_sql(&format!(include_str!("../schema.sql"), db_database=db_database, votebase_server_password=votebase_server_password))
		.execute(&pool).await.expect("wasn't able to execute schema.sql");

	votebase_common::runtime::create_ruleset(
		&pool, None, "root",
		&vec!["__insert_initial".to_string()], &vec![],
		include_str!("../../rulesets/accept-any/ruleset.ts"), "",
	).await.expect("wasn't able to create root seed ruleset");

	println!("{votebase_server_password}");

	Ok(())
}
