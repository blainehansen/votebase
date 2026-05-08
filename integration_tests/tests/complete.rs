use tokio::process::Command;

// TODO create a sync spawn_postgres that uses a std::process::child instead?
#[tokio::test]
async fn test_total_votebase_workflow() -> Result<(), Box<dyn std::error::Error>> {
	let bin_votebase_cli = env!("CARGO_BIN_EXE_votebase_cli");
	let bin_votebase_bootstrap_cli = env!("CARGO_BIN_EXE_votebase_bootstrap_cli");
	let bin_votebase_server = env!("CARGO_BIN_EXE_votebase_server");

	// run bin_votebase_cli to package up a ruleset
	let output = tokio::process::Command::new(bin_votebase_cli)
		.args(&["bundle", "test-rulesets/yes-or-no", "test-rulesets-bundled/yes-or-no.json"])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bundle {}", String::from_utf8_lossy(&output.stderr));

	// start the database container in the background
	let (_, config, pg_pass, pg_user, pg_db, pg_port) = utils::generate_temp_config();
	let _postgres_process = utils::spawn_postgres("test_total_votebase_workflow", &pg_pass, &pg_user, &pg_db, pg_port)?;

	// run bin_votebase_bootstrap_cli to initialize the schema and seed the packaged root ruleset

	let db_url = utils::url_encoded_connection_string(&config);
	let output = tokio::process::Command::new(bin_votebase_bootstrap_cli)
		.args(&[&db_url, "test-rulesets-bundled/yes-or-no.json"])
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?.wait_with_output().await?;
	assert!(output.status.success(), "Failed bootstrap {}", String::from_utf8_lossy(&output.stderr));
	let server_password = String::from_utf8(output.stdout)?;

	// start bin_votebase_server in the background
	use utils::GracefulSpawn;
	let _server_process = Command::new(bin_votebase_server)
		// TODO needs envs for port and db etc
		// .env(key, val)
		// TODO
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.graceful_spawn()?;

	// run http requests against the server and check their outputs and the contents of the database
	// curl? reqwest?
	// https://github.com/algesten/ureq

	// assert!(output_b.status.success(), "crate-b failed: {:?}", output_b.stderr);

	Ok(())
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
