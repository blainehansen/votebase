use tokio::{process::Command};
use utils::tokio_graceful_spawn::GracefulChild;

fn format_ruleset(ruleset_name: &str) -> String {
	format!("../test-rulesets/{ruleset_name}")
}
fn format_bundled_ruleset(ruleset_name: &str) -> String {
	format!("../test-rulesets/{ruleset_name}.bundled.json")
}

async fn setup_initial_ruleset(ruleset_name: &str) -> Result<(GracefulChild, GracefulChild), Box<dyn std::error::Error>> {
	let ruleset_bundle = bundle_ruleset(&ruleset_name).await?;

	// start the database container in the background
	let (container_name, config, pg_pass, pg_user, pg_db, pg_port) = utils::temp_containers::generate_temp_config();
	let postgres_process =
		utils::temp_containers::spawn_postgres_tokio(&container_name, &pg_pass, &pg_user, &pg_db, pg_port)?
		.debug_output();
	utils::temp_containers::healthcheck_postgres(&config, 50, 100).await?;

	// run votebase_bootstrap_cli
	let db_url = utils::url_encoded_connection_string(&config);
	let output = Command::new(votebase_bin("votebase_bootstrap_cli"))
		.args(&[&db_url, &ruleset_bundle])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bootstrap {}", String::from_utf8_lossy(&output.stderr));
	let server_password = String::from_utf8(output.stdout)?;

	// start votebase_server in the background
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

async fn bundle_ruleset(ruleset_name: &str) -> Result<String, Box<dyn std::error::Error>> {
	let ruleset_path = format_ruleset(ruleset_name);
	let ruleset_bundle = format_bundled_ruleset(ruleset_name);

	let output = Command::new(votebase_bin("votebase_cli"))
		.args(&[&ruleset_path, "bundle", &ruleset_bundle])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bundle {}", String::from_utf8_lossy(&output.stderr));

	Ok(ruleset_bundle)
}

// tests both replacement and the general bundle/bootstrap/view/action functionality
#[tokio::test]
#[serial_test::serial]
async fn test_total_accept_any_to_no_db() -> Result<(), Box<dyn std::error::Error>> {
	let (_postgres_process, _server_process) = setup_initial_ruleset("accept-any").await?;

	let client = reqwest::Client::new();
	let resp: Vec<String> = client.get("http://localhost:8080/rulesets").send().await?.json().await?;
	assert_eq!(resp, vec!["root"]);

	let no_db_bundle_path = bundle_ruleset("no-db").await?;
	let no_db_bundle = tokio::fs::read_to_string(no_db_bundle_path).await?;
	let resp = client.post("http://localhost:8080/fn/action/root|replaceSelf")
		.header("Content-Type", "application/json")
		.body(no_db_bundle)
		.send().await?.status();
	assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);

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
