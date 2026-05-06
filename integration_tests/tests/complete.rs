use tokio::process::Command;

// TODO create a sync spawn_postgres that uses a std::process::child instead?
#[tokio::test]
async fn test_total_votebase_workflow() {
	let bin_votebase_cli = env!("CARGO_BIN_EXE_votebase_cli");
	let bin_votebase_bootstrap_cli = env!("CARGO_BIN_EXE_votebase_bootstrap_cli");
	let bin_votebase_server = env!("CARGO_BIN_EXE_votebase_server");

	// run bin_votebase_cli to package up a ruleset

	// start the database container in the background
	let (_, config, pg_pass, pg_user, pg_db, pg_port) = temp_container_utils::generate_temp_config();
	let _postgres_process = temp_container_utils::spawn_postgres("total_votebase_workflow", &pg_pass, &pg_user, &pg_db, pg_port)?;

	// run bin_votebase_bootstrap_cli to initialize the schema and seed the packaged root ruleset

	// start bin_votebase_server in the background
	use temp_container_utils::GracefulSpawn;
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
