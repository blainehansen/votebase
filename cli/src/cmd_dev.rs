use std::path::Path;

// - in a temp podman postgres
//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
//   - generate the queries and write them into the file
pub async fn cmd_dev(ruleset_dir: &Path) -> anyhow::Result<()> {
	crate::run_sql_checking_and_generation(&ruleset_dir, true).await
}

#[cfg(test)]
mod tests {
	use super::*;
	use assert_fs::prelude::*;
	use predicates::prelude::*;

	#[test]
	fn test_cmd_dev_test_rulesets() {
		use predicate::path;

		let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

		for entry in std::fs::read_dir("../test-rulesets").unwrap() {
			let entry = entry.unwrap().path();
			if !entry.is_dir() { continue; }

			let temp_dir = assert_fs::TempDir::new().unwrap();
			temp_dir.copy_from(&entry, &["**", "!queries.ts"]).unwrap();
			temp_dir.child("queries.ts").assert(path::missing());
			let temp_dir_path = temp_dir.path();

			runtime.block_on(cmd_dev(temp_dir_path)).unwrap();

			let expected_queries_path = entry.with_extension("expected.queries.ts");
			// agh, this is to give a little bit of time? frustratingly the assertion doesn't always pick up the new contents in time?
			assert!(runtime.block_on(tokio::fs::try_exists(&expected_queries_path)).unwrap());
			// print!("{}", tokio::fs::read_to_string(entry.with_extension("expected.queries.ts")).await.unwrap());
			temp_dir.child("queries.ts").assert(path::eq_file(&expected_queries_path));

			runtime.block_on(crate::run_votebase_tsc(temp_dir_path)).unwrap();
		}
	}
}
