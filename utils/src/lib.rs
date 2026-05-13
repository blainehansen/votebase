pub mod temp_containers;


pub mod tokio_graceful_spawn {
	pub struct GracefulChild {
		pub inner: Option<tokio::process::Child>,
	}
	impl GracefulChild {
		pub fn inner_mut(&mut self) -> Option<&mut tokio::process::Child> {
			self.inner.as_mut()
		}
		pub fn debug_output(mut self) -> GracefulChild {
			let stdout = self.inner.as_mut().unwrap().stdout.take().unwrap();
			tokio::spawn(async move {
				use tokio::io::AsyncBufReadExt;
				let mut reader = tokio::io::BufReader::new(stdout).lines();

				while let Ok(Some(line)) = reader.next_line().await {
					println!("stdout: {}", line);
				}
			});
			let stderr = self.inner.as_mut().unwrap().stderr.take().unwrap();
			tokio::spawn(async move {
				use tokio::io::AsyncBufReadExt;
				let mut reader = tokio::io::BufReader::new(stderr).lines();

				while let Ok(Some(line)) = reader.next_line().await {
					println!("stderr: {}", line);
				}
			});
			self
		}
	}
	impl Drop for GracefulChild {
		fn drop(&mut self) {
			if let Some(child) = self.inner.take() {
				if let Some(pid) = child.id() {
					let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGINT);
				}
				tokio::spawn(async move {
					let mut child = child;
					let _ = child.wait().await;
				});
			}
		}
	}

	pub trait GracefulSpawn {
		fn graceful_spawn(&mut self) -> std::io::Result<GracefulChild>;
	}
	impl GracefulSpawn for tokio::process::Command {
		fn graceful_spawn(&mut self) -> std::io::Result<GracefulChild> {
			let child = self.kill_on_drop(false).spawn()?;
			Ok(GracefulChild { inner: Some(child) })
		}
	}
}

pub mod std_graceful_spawn {
	pub struct GracefulChild {
		pub inner: Option<std::process::Child>,
	}

	impl GracefulChild {
		pub fn inner_mut(&mut self) -> Option<&mut std::process::Child> {
			self.inner.as_mut()
		}

		pub fn debug_output(mut self) -> GracefulChild {
			let stdout = self.inner.as_mut().unwrap().stdout.take().unwrap();
			std::thread::spawn(move || {
				use std::io::BufRead;
				let mut reader = std::io::BufReader::new(stdout).lines();

				while let Some(Ok(line)) = reader.next() {
					println!("stdout: {}", line);
				}
			});
			let stderr = self.inner.as_mut().unwrap().stderr.take().unwrap();
			std::thread::spawn(move || {
				use std::io::BufRead;
				let mut reader = std::io::BufReader::new(stderr).lines();

				while let Some(Ok(line)) = reader.next() {
					println!("stderr: {}", line);
				}
			});
			self
		}
	}

	impl Drop for GracefulChild {
		fn drop(&mut self) {
			if let Some(mut child) = self.inner.take() {
				let pid = child.id();
				let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGINT);
				let _ = child.wait();
			}
		}
	}

	pub trait GracefulSpawn {
		fn graceful_spawn(&mut self) -> std::io::Result<GracefulChild>;
	}
	impl GracefulSpawn for std::process::Command {
		fn graceful_spawn(&mut self) -> std::io::Result<GracefulChild> {
			let child = self.spawn()?;
			Ok(GracefulChild { inner: Some(child) })
		}
	}
}


pub fn url_encoded_connection_string(config: &tokio_postgres::Config) -> String {
	use percent_encoding::{utf8_percent_encode, percent_encode, NON_ALPHANUMERIC};
	let mut url = String::from("postgresql://");

	if let Some(user) = config.get_user() {
		url.push_str(&utf8_percent_encode(user, NON_ALPHANUMERIC).to_string());
	}

	if let Some(password_bytes) = config.get_password() {
		url.push(':');
		url.push_str(&percent_encode(password_bytes, NON_ALPHANUMERIC).to_string());
	}

	let hosts = config.get_hosts();
	let ports = config.get_ports();
	if !hosts.is_empty() {
		url.push('@');
		for (i, host) in hosts.iter().enumerate() {
			if i > 0 {
				url.push(',');
			}
			use tokio_postgres::config::Host;
			match host {
				Host::Tcp(hostname) => {
					url.push_str(&utf8_percent_encode(hostname, NON_ALPHANUMERIC).to_string());
				}
				#[cfg(unix)]
				Host::Unix(path) => {
					use std::os::unix::ffi::OsStrExt;
					let path_bytes = path.as_os_str().as_bytes();
					url.push_str(&percent_encode(&path_bytes, NON_ALPHANUMERIC).to_string());
				}
			}

			if let Some(&port) = ports.get(i) {
				url.push(':');
				url.push_str(&port.to_string());
			}
		}
	}

	if let Some(dbname) = config.get_dbname() {
		url.push('/');
		url.push_str(&utf8_percent_encode(dbname, NON_ALPHANUMERIC).to_string());
	}

	url
}

#[test]
fn test_url_encoded_connection_string() {
	let config = "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db".parse().unwrap();
	assert_eq!(url_encoded_connection_string(&config), "postgresql://dev%5Fadmin%5Fuser:dev%5Fadmin%5Fpassword@localhost:5432/dev%5Fdb");
}

// #[derive(thiserror::Error, Debug)]
// pub enum ConnectionStringError {
// 	#[error("no user on config?")]
// 	NoUser,
// 	#[error("no host on config?")]
// 	NoHost,
// 	#[error("no port on config?")]
// 	NoPort,
// 	#[error("no dbname on config?")]
// 	NoDbname,
// }


// fn url_encoded_connection_string(config: &Config) -> Result<String, ConnectionStringError> {
// 	use percent_encoding::{utf8_percent_encode, percent_encode, NON_ALPHANUMERIC};

// 	let mut url = String::from("postgresql://");

// 	let user = config.get_user().ok_or(ConnectionStringError::NoUser)?;
// 	url.push_str(&utf8_percent_encode(user, NON_ALPHANUMERIC).to_string());

// 	if let Some(password_bytes) = config.get_password() {
// 		url.push(':');
// 		url.push_str(&percent_encode(password_bytes, NON_ALPHANUMERIC).to_string());
// 	}

// 	let host = config.get_hosts().get(0).ok_or(ConnectionStringError::NoHost)?;
// 	url.push('@');
// 	use votebase_common::postgres::config::Host;
// 	match host {
// 		Host::Tcp(hostname) => {
// 			url.push_str(&utf8_percent_encode(hostname, NON_ALPHANUMERIC).to_string());
// 		}
// 		#[cfg(unix)]
// 		Host::Unix(path) => {
// 			use std::os::unix::ffi::OsStrExt;
// 			let path_bytes = path.as_os_str().as_bytes();
// 			url.push_str(&percent_encode(&path_bytes, NON_ALPHANUMERIC).to_string());
// 		}
// 	}

// 	let port = config.get_ports().get(0).ok_or(ConnectionStringError::NoPort)?;
// 	url.push(':');
// 	url.push_str(&port.to_string());

// 	let dbname = config.get_dbname().ok_or(ConnectionStringError::NoDbname)?;
// 	url.push('/');
// 	url.push_str(&utf8_percent_encode(dbname, NON_ALPHANUMERIC).to_string());

// 	Ok(url)
// }


pub enum JoinOption<T1, T2> {
	Left(T1),
	Right(T2),
	Both(T1, T2),
}

pub fn outer_join<'a, K: Eq + std::hash::Hash, V1, V2>(
	map1: &'a std::collections::HashMap<K, V1>,
	map2: &'a std::collections::HashMap<K, V2>
) -> std::collections::HashMap<&'a K, JoinOption<&'a V1, &'a V2>> {
	let mut result = std::collections::HashMap::new();
	use JoinOption::*;

	for (k, v) in map1 {
		result.insert(k, Left(v));
	}
	for (k, v) in map2 {
		match result.entry(k) {
			std::collections::hash_map::Entry::Occupied(mut entry) => {
				if let Left(l) = entry.get() {
					entry.insert(Both(l, v));
				}
				else { unreachable!() };
			},
			std::collections::hash_map::Entry::Vacant(entry) => {
				entry.insert(Right(v));
			},
		}
	}

	result
}
