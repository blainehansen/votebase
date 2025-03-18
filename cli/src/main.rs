#[tokio::main]
async fn main() -> std::io::Result<()> {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let admin_connection_string = args.get(0).expect("only parameter should be a database url");

	let db_url: sqlx::postgres::PgConnectOptions = admin_connection_string.parse().expect("invalid db url");
	let db_database = db_url.get_database().unwrap();
	let pool = sqlx::PgPool::connect(&db_database).await.expect("unable to connect");

	let ruleset_count = sqlx::query!(r#"select coalesce(count("name"), 0) as "ruleset_count!" from votebase_catalog.ruleset;"#)
		.fetch_one(&pool).await.expect("wasn't able to fetch the count of rulesets").ruleset_count;

	if ruleset_count != 0 {
		return Err(std::io::Error::new(
			std::io::ErrorKind::AlreadyExists,
			format!("the votebase database already has {ruleset_count} rulesets"),
		));
	}

	sqlx::raw_sql(&format!(include_str!("../schema.sql"), db_database=db_database))
		.execute(&pool).await.expect("wasn't able to execute schema.sql");

	// TODO need to move all the runtime stuff to common, and rename it to something like "common" or whatever
	// then this can be shared
	votebase_common::runtime::create_ruleset(
		&pool, None, "root",
		&vec!["__insert_initial".to_string()], &vec![],
		include_str!("../../rulesets/accept-any/ruleset.ts"), "",
	).await.expect("wasn't able to create root seed ruleset");

	Ok(())
}
