use std::path::Path;

pub fn cmd_init(ruleset_dir: &Path) -> std::io::Result<()> {
	// if !ruleset_dir.is_dir() {
	// 	return Err(std::io::Error::other(format!("{ruleset_dir} must be a ")))
	// }

	// let package_name = ruleset_dir.file_name();
	// https://crates.io/crates/handlebars

	// inlining basically the include_dir::Dir::extract function so that if we need to do any templating we can
	fn extract_dir(dir: &include_dir::Dir<'_>, base_path: impl AsRef<Path>) -> std::io::Result<()> {
		use std::fs;
		let base_path = base_path.as_ref();

		for entry in dir.entries() {
			let path = base_path.join(entry.path());
			match entry {
				include_dir::DirEntry::Dir(d) => {
					fs::create_dir_all(&path)?;
					extract_dir(d, base_path)?;
				}
				include_dir::DirEntry::File(f) => {
					fs::write(path, f.contents())?;
				}
			}
		}

		Ok(())
	}

	static PROJECT_DIR: include_dir::Dir<'_> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/init_template");
	extract_dir(&PROJECT_DIR, ruleset_dir)
}

#[cfg(test)]
mod tests {
	use super::*;
	use assert_fs::prelude::*;
	use predicates::prelude::*;

	#[test]
	fn test_cmd_init_same_as_template() {
		use predicate::path;

		let temp_dir = assert_fs::TempDir::new().unwrap();
		cmd_init(temp_dir.path()).unwrap();

		for entry in walkdir::WalkDir::new(temp_dir.path()) {
			let entry = entry.unwrap();
			if !entry.file_type().is_file() { continue; }

			temp_dir.child(entry.path())
				.assert(path::exists())
				.assert(path::eq_file(Path::new("init_template").join(entry.path())));
		}
	}
}

