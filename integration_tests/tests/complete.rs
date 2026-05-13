use tokio::{process::Command};
use utils::tokio_graceful_spawn::GracefulChild;

async fn total_setup(initial_test_ruleset_name: &str) -> Result<(GracefulChild, GracefulChild), Box<dyn std::error::Error>> {
	let ruleset_path = format!("../test-rulesets/{initial_test_ruleset_name}");
	let ruleset_bundle = format!("../test-rulesets/{initial_test_ruleset_name}.bundled.json");

	let output = Command::new(votebase_bin("votebase_cli"))
		.args(&[&ruleset_path, "bundle", &ruleset_bundle])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bundle {}", String::from_utf8_lossy(&output.stderr));

	// start the database container in the background
	let (_, config, pg_pass, pg_user, pg_db, pg_port) = utils::temp_containers::generate_temp_config();
	let postgres_process =
		utils::temp_containers::spawn_postgres_tokio("test_total_no_db", &pg_pass, &pg_user, &pg_db, pg_port)?
		.debug_output();
	utils::temp_containers::healthcheck_postgres(&config, 50, 100).await?;

	// run bin_votebase_bootstrap_cli to initialize the schema and seed the packaged root ruleset
	let db_url = utils::url_encoded_connection_string(&config);
	let output = Command::new(votebase_bin("votebase_bootstrap_cli"))
		.args(&[&db_url, &ruleset_bundle])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bootstrap {}", String::from_utf8_lossy(&output.stderr));
	let server_password = String::from_utf8(output.stdout)?;

	// start bin_votebase_server in the background
	use utils::tokio_graceful_spawn::GracefulSpawn;
	let server_process = Command::new(votebase_bin("votebase_server"))
		.envs([
			("DB_HOST", "localhost"),
			("DB_PORT", pg_port.to_string().as_str()),
			("DB_NAME", &pg_db),
			("VOTEBASE_SERVER_PASSWORD", server_password.trim()),
			("DATABASE_MAX_CONNECTIONS", 2.to_string().as_str()),
			("VOTEBASE_HOST", "0.0.0.0"),
			("VOTEBASE_PORT", "8080"),
		])
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.graceful_spawn()?.debug_output();

	Ok((postgres_process, server_process))
}

#[tokio::test]
#[serial_test::serial]
async fn test_total_no_db() -> Result<(), Box<dyn std::error::Error>> {
	let (_postgres_process, _server_process) = total_setup("no-db").await?;

	let client = reqwest::Client::new();
	let resp: Vec<String> = client.get("http://localhost:8080/rulesets").send().await?.json().await?;
	assert_eq!(resp, vec!["root"]);

	// the no_db ruleset is fundamentally flawed, because the ephemeral counter will be reset on every run!
	// this is the sort of thing I'd love to warn people of in the future using a flow effects system
	let resp: u32 = client.get("http://localhost:8080/fn/view/root|seeCounter").send().await?.json().await?;
	assert_eq!(resp, 0);

	let resp = client.post("http://localhost:8080/fn/action/root|decCounter").json(&serde_json::json!(null)).send().await?.status();
	assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);
	let resp: u32 = client.get("http://localhost:8080/fn/view/root|seeCounter").send().await?.json().await?;
	assert_eq!(resp, 0);

	let resp = client.post("http://localhost:8080/fn/action/root|incCounter").json(&serde_json::json!(null)).send().await?.status();
	assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);
	let resp: u32 = client.get("http://localhost:8080/fn/view/root|seeCounter").send().await?.json().await?;
	assert_eq!(resp, 0);

	let resp = client.post("http://localhost:8080/fn/action/root|decCounter").json(&serde_json::json!(null)).send().await?.status();
	assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);

	Ok(())
}


fn votebase_bin(c: &str) -> std::path::PathBuf {
	let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	path.pop();
	path.push("target");
	path.push("debug");
	path.push(c);
	path
}
