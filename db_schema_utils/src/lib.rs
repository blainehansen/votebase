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

pub async fn load_votebase_server_schema(
	db_name: &str,
	client: &impl tokio_postgres::GenericClient,
) -> Result<String, tokio_postgres::Error> {
	let votebase_server_password = generate_votebase_server_pass();
	let schema_sql = format!(include_str!("../schema.sql"), db_name=db_name, votebase_server_password=votebase_server_password);
	client.batch_execute(&schema_sql).await?;

	Ok(votebase_server_password)
}
