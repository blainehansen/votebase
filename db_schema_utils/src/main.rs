#[tokio::main]
async fn main() {
	use clorinde::config::Config;
	let config = Config::builder_from_file("db_schema_utils/clorinde.toml").unwrap()
		.queries("db_schema_utils/queries")
		.destination("db_generated")
		// .async(true)
		.build();

	temp_container_utils::with_temp_postgres_client(async |_, db_config, mut client| {
		let db_name = db_config.get_dbname().unwrap();
		db_schema_utils::load_votebase_server_schema(db_name, &mut client).await.unwrap();

		clorinde::gen_live(&client, config).unwrap();
	}).await.unwrap();
}
