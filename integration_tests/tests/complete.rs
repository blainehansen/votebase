use tokio::{process::Command};

#[tokio::test]
async fn test_total_no_db() -> Result<(), Box<dyn std::error::Error>> {
	let output = Command::new(votebase_bin("votebase_cli"))
		.args(&["../test-rulesets/no-db", "bundle", "../test-rulesets/no-db.bundled.json"])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bundle {}", String::from_utf8_lossy(&output.stderr));

	// start the database container in the background
	let (_, config, pg_pass, pg_user, pg_db, pg_port) = utils::temp_containers::generate_temp_config();
	let mut postgres_process = utils::temp_containers::spawn_postgres_tokio("test_total_no_db", &pg_pass, &pg_user, &pg_db, pg_port)?;
	utils::temp_containers::healthcheck_postgres(&config, 50, 100).await?;
	// let stdout = postgres_process.inner_mut().unwrap().stdout.take().unwrap();
	// tokio::spawn(async move {
	// 	use tokio::io::AsyncBufReadExt;
	// 	let mut reader = tokio::io::BufReader::new(stdout).lines();

	// 	while let Some(line) = reader.next_line().await.unwrap() {
	// 		println!("Captured: {}", line);
	// 	}
	// });
	// let stderr = postgres_process.inner_mut().unwrap().stderr.take().unwrap();
	// tokio::spawn(async move {
	// 	use tokio::io::AsyncBufReadExt;
	// 	let mut reader = tokio::io::BufReader::new(stderr).lines();

	// 	while let Some(line) = reader.next_line().await.unwrap() {
	// 		println!("Captured: {}", line);
	// 	}
	// });

	// run bin_votebase_bootstrap_cli to initialize the schema and seed the packaged root ruleset
	let db_url = utils::url_encoded_connection_string(&config);
	let output = Command::new(votebase_bin("votebase_bootstrap_cli"))
		.args(&[&db_url, "../test-rulesets/no-db.bundled.json"])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bootstrap {}", String::from_utf8_lossy(&output.stderr));

	// start bin_votebase_server in the background
	use utils::tokio_graceful_spawn::GracefulSpawn;
	let mut server_process = Command::new(votebase_bin("votebase_server"))
		.envs([
			("DB_HOST", "localhost"),
			("DB_PORT", pg_port.to_string().as_str()),
			("DB_NAME", &pg_db),
			("VOTEBASE_SERVER_PASSWORD", "votebase_server_dev_pass"),
			("DATABASE_MAX_CONNECTIONS", 2.to_string().as_str()),
			("VOTEBASE_HOST", "0.0.0.0"),
			("VOTEBASE_PORT", "8080"),
		])
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.graceful_spawn()?;

	// let stdout = server_process.inner_mut().unwrap().stdout.take().unwrap();
	// tokio::spawn(async move {
	// 	use tokio::io::AsyncBufReadExt;
	// 	let mut reader = tokio::io::BufReader::new(stdout).lines();

	// 	while let Some(line) = reader.next_line().await.unwrap() {
	// 		println!("Captured: {}", line);
	// 	}
	// });
	// let stderr = server_process.inner_mut().unwrap().stderr.take().unwrap();
	// tokio::spawn(async move {
	// 	use tokio::io::AsyncBufReadExt;
	// 	let mut reader = tokio::io::BufReader::new(stderr).lines();

	// 	while let Some(line) = reader.next_line().await.unwrap() {
	// 		println!("Captured: {}", line);
	// 	}
	// });

	tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
	let client = reqwest::Client::new();
	println!("rulesets");
	let resp: Vec<String> = client.get("http://localhost:8080/rulesets").send().await?.json().await?;
	assert_eq!(resp, vec!["root"]);

	// the no_db ruleset is fundamentally flawed, because the ephemeral counter will be reset on every run!
	// this is the sort of thing I'd love to warn people of in the future using a flow effects system
	println!("seeCounter");
	let resp: u32 = client.get("http://localhost:8080/fn/view/root|seeCounter").send().await?.json().await?;
	assert_eq!(resp, 0);

	println!("decCounter");
	let resp = client.post("http://localhost:8080/fn/action/root|decCounter").send().await?.status(); assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);
	println!("seeCounter");
	let resp: u32 = client.get("http://localhost:8080/fn/view/root|seeCounter").send().await?.json().await?;
	assert_eq!(resp, 0);

	println!("incCounter");
	let resp = client.post("http://localhost:8080/fn/action/root|incCounter").send().await?.status(); assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);
	println!("seeCounter");
	let resp: u32 = client.get("http://localhost:8080/fn/view/root|seeCounter").send().await?.json().await?;
	assert_eq!(resp, 0);

	println!("decCounter");
	let resp = client.post("http://localhost:8080/fn/action/root|decCounter").send().await?.status(); assert_eq!(resp, reqwest::StatusCode::NO_CONTENT);
	assert_eq!(resp, 0);

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


// pub struct GracefulChild {
// 	inner: Option<std::process::Child>,
// }
// impl GracefulChild {
// 	pub fn inner_mut(&mut self) -> Option<&mut std::process::Child> {
// 		self.inner.as_mut()
// 	}
// }
// impl Drop for GracefulChild {
// 	fn drop(&mut self) {
// 		if let Some(mut child) = self.inner.take() {
// 			let pid = child.id();
// 			let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGINT);
// 			let _ = child.wait();
// 		}
// 	}
// }

// pub trait GracefulSpawn {
// 	fn graceful_spawn(&mut self) -> std::io::Result<GracefulChild>;
// }
// impl GracefulSpawn for std::process::Command {
// 	fn graceful_spawn(&mut self) -> std::io::Result<GracefulChild> {
// 		let child = self.spawn()?;
// 		Ok(GracefulChild { inner: Some(child) })
// 	}
// }
