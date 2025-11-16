// - in a temp podman postgres
//   - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
//   - generate the queries and write them into the file

use std::{collections::HashMap, path::PathBuf};


#[derive(Debug)]
enum FileSystem {
	ActualFiles,
	Stored(HashMap<PathBuf, String>),
}

impl FileSystem {
	async fn get_file_content(&self, path_buf: &PathBuf) -> std::io::Result<String> {
		match self {
			FileSystem::ActualFiles => {
				tokio::fs::read_to_string(p).await
			},
			FileSystem::Stored(files_map) => {
				let content = files_map.get(path_buf).ok_or_else(|| std::io::Error::(format!("")))?;
				std::future::ready(Ok(content.to_string()))
			},
		}
	}
}

type FileMap = HashMap<PathBuf, Fn() -> Future<String>>;

async fn do_dev(ruleset_files: FileMap) -> Result<(), String> {
	temp_container_utils::with_temp_postgres_client(async |db_config, mut client| {
		let db_name = db_config.get_dbname().unwrap().to_string();
		db_schema_utils::load_votebase_server_schema(db_name, &mut client).await?;

		// now do all the query generation
	}).await?;

	Ok(())
}

// let generated = {
// 	votebase_common::runtime::create_ruleset(
// 		&config, &mut temp_client,
// 		None, "root",
// 		&vec!["__insert_initial".to_string()], &vec![],
// 		include_str!("../../rulesets/accept-any/ruleset.ts"), "",
// 	).await?;

// 	let db_schema = tokio::fs::read_to_string(ruleset_dir.join("schema.sql")).await?;
// 	temp_client.batch_execute(&db_schema).await?;

// 	let generated_fields = votebase_common::gen_queries::generate_queries(queries_dir.clone(), &temp_client).await?;
// 	format!("import 'votebase'\nexport default {{\n{generated_fields}\n}}")
// };
// client.batch_execute(&format!(r#"drop database if exists "{temp_dbname}""#)).await?;

// let mut file = tokio::fs::OpenOptions::new().write(true).create(true)
// 	.open(format!("{}.ts", queries_dir.to_string_lossy())).await?;

// file.write_all(generated.as_bytes()).await?;



// run_dev

// run_check

// run_bundle

// validate_packaged_ruleset, meaning looking at a ruleset that should be fully ready to slot in somewhere, and say whether it's right

// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	#[test]
// 	fn name() {
// 		unimplemented!();
// 	}
// }
