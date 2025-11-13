#[tokio::main]
async fn main() {
	deno_core::extension!(
		// extension name
		votebase,
		// list of all JS files in the extension
		esm_entry_point = "ext:votebase/src/runtime/runtime.ts",
		// the entrypoint to our extension
		esm = ["src/runtime/runtime.ts"]
	);

	let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
	let snapshot_path = out_dir.join("VOTEBASE_SNAPSHOT.bin");

	let snapshot = deno_core::snapshot::create_snapshot(
		deno_core::snapshot::CreateSnapshotOptions {
			cargo_manifest_dir: std::env!("CARGO_MANIFEST_DIR"),
			startup_snapshot: None,
			skip_op_registration: false,
			extensions: vec![votebase::init()],
			with_runtime_cb: None,
			extension_transpiler: Some(std::rc::Rc::new(|module_name, module_code| {
				votebase_transpile::transpile_typescript(module_name, module_code)
					.map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))
			})),
		},
		None,
	)
	.unwrap();

	std::fs::write(snapshot_path, snapshot.output).unwrap();

	use clorinde::config::Config;
	let queries_path = "queries";
	let schema_file = "schema.sql";
	println!("cargo:rerun-if-changed={queries_path}");
	println!("cargo:rerun-if-changed={schema_file}");

	let config = Config::builder_from_file("clorinde.toml").unwrap()
		.queries(queries_path)
		.destination("generated_queries")
		// .async(true)
		.build();

	temp_containers::with_temp_postgres_client(async |db_config, mut client| {
		let db_name = db_config.get_dbname().unwrap().to_string();
		load_votebase_server_schema(db_name, &mut client).await.unwrap();

		clorinde::gen_live(&client, config).unwrap();
	}).await.unwrap();
}


pub fn generate_votebase_server_pass() -> String {
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

	votebase_server_password
}

type AnyError = Box<dyn std::error::Error>;
pub async fn load_votebase_server_schema(db_name: String, client: &mut tokio_postgres::Client) -> Result<String, AnyError> {
	let votebase_server_password = generate_votebase_server_pass();
	let schema_sql = format!(include_str!("./schema.sql"), db_name=db_name, votebase_server_password=votebase_server_password);
	client.batch_execute(&schema_sql).await?;

	Ok(votebase_server_password)
}
