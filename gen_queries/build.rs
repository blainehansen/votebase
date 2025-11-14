// TODO maybe truly just merge this into common? given that the trick to have it bootstrap cleanly didn't work, just lean into it and remove the layer of indirection that is gen_queries

#[tokio::main]
async fn main() {
	use clorinde::config::Config;
	let queries_path = "queries";
	let schema_file = "../db_schema.sql";
	println!("cargo:rerun-if-changed={queries_path}");
	println!("cargo:rerun-if-changed={schema_file}");
	println!("cargo:rerun-if-changed=clorinde.toml");

	let config = Config::builder_from_file("clorinde.toml").unwrap()
		.queries(queries_path)
		.destination("auto_gen_queries")
		// .async(true)
		.build();

	temp_container_utils::with_temp_postgres_client(async |db_config, mut client| {
		let db_name = db_config.get_dbname().unwrap().to_string();
		db_schema_utils::load_votebase_server_schema(db_name, &mut client).await.unwrap();

		clorinde::gen_live(&client, config).unwrap();
	}).await.unwrap();
}
