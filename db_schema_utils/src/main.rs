#[tokio::main]
async fn main() {
	use clorinde::config::Config;
	let config = Config::builder_from_file("db_schema_utils/clorinde.toml").unwrap()
		.queries("db_schema_utils/queries")
		.destination("db_generated")
		// .async(true)
		.build();

	utils::temp_containers::with_temp_postgres_client(async |_, db_config, mut client| -> anyhow::Result<()> {
		let db_name = db_config.get_dbname().ok_or(anyhow::anyhow!("no dbname"))?;
		db_schema_utils::load_votebase_server_schema(db_name, &mut client).await?;

		clorinde::gen_live(&client, config)?;

		Ok(())
	}).await.unwrap().unwrap();
}
