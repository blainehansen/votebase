use std::collections::HashMap;
use assert_fs::prelude::PathChild;

use crate::rulesets::{
	ValidateCtx, BundledRuleset, StoredRuleset,
	create_ruleset, apply_candidate, validate_bundled_ruleset,
	propose_candidate_ruleset, apply_candidate_top,
};


// tests for validation (which don't require me to set up a temp podman) and application (which do)
// validation and application should always be paired in testing, since they're basically the same thing just with different setup etc
// so really this will be a bunch of cases of Option<StoredRuleset> against BundledRuleset, some of which are intended to be valid and successful, and a different class which are intended to be failures. the invariant we're trying to achieve is that validation succeeding *implies* application succeeding

// #[tokio::test]
// async fn test_valid_ruleset_new() -> Result<(), _> {
// 	let new_rulesets: Vec<BundledRuleset> = vec![];

// 	for new_ruleset in new_rulesets {
// 		validate_bundled_ruleset_top(parent_full_path, ruleset_name, full_path, None, next, server_db_archive_path).await?;
// 		utils::temp_containers::with_temp_postgres_client(async |db_container_name, config, client| {
// 			apply_candidate(config, server_role_tx, parent_full_path, ruleset_name, full_path, prev, bundled_ruleset).await?;

// 			Ok(())
// 		}).await??;
// 	}

// 	Ok(())
// }

#[tokio::test]
async fn test_valid_ruleset_transitions() -> anyhow::Result<()> {
	// from no children to uses (in all ways) parent
	// from no children to uses (in all ways) grand parent
	// from no children to uses (in all ways) uncle
	// from no children to uses (in all ways) grand uncle

	// from no children to ruleset uses (in all ways) sibling
	// from no children to ruleset uses (in all ways) cousin
	let transitions: Vec<(StoredRuleset, BundledRuleset)> = vec![
		(
			StoredRuleset {
				ts_code: "".to_string(),
				db_schema: r#""#.to_string(),
				db_uses_functions: vec![],
				db_uses_tables: vec![],
				static_children: HashMap::from([]),
			},
			BundledRuleset {
				ts_code: "".to_string(),
				db_schema: r#""#.to_string(),
				db_migration: r#""#.to_string(),
				db_uses_functions: vec![],
				db_uses_tables: vec![],
				static_children: HashMap::from([]),
			},
		),
	];

	for (prev, next) in transitions {

		utils::temp_containers::with_temp_postgres_client(async |db_container_name, config, client| {
			let ruleset_name = "root";
			let ctx = ValidateCtx {
				db_container_name,
				reset_server_client, reset_config,
				migrate_server_client, migrate_config,
			};

			validate_bundled_ruleset(ruleset_name, None, ruleset_name, &ctx, Some(&prev), &next).await?;

			// set up database to already have prev
			create_ruleset(&config, &mut client, None, "root", &prev.ts_code, db_schema, fns, function_uses, table_uses).await?;
			let mut server_role_tx = client.transaction().await?;
			apply_candidate(&config, &mut server_role_tx, None, ruleset_name, ruleset_name, Some(prev), next).await?;

			Ok(())
		}).await??;
	}

	Ok(())
}

// #[tokio::test]
// async fn test_valid_ruleset_transition_paths() -> anyhow::Result<()> {
// 	let paths: Vec<(StoredRuleset, Vec<BundledRuleset>)> = vec![
// 		// from no children to ruleset uses (in all ways) sibling bi-directionally through pivot state
// 	];

// 	for (prev, transition_path) in paths {
// 		utils::temp_containers::with_temp_postgres_client(async |db_container_name, config, client| {
// 			let ruleset_name = "root";
// 			let ctx = ValidateCtx {
// 				db_container_name,
// 				reset_server_client, reset_config,
// 				migrate_server_client, migrate_config,
// 			};

// 			// set up database to already have prev
// 			create_ruleset(
// 				&config, &mut client, None, ruleset_name, &prev.ts_code, &prev.db_schema,
// 				prev.fns, &prev.db_uses_functions, &prev.db_uses_tables,
// 			).await?;
// 			let mut server_role_tx = client.transaction().await?;

// 			for next in transition_path.into_iter() {
// 				let server_db_archive_path = assert_fs::NamedTempFile::new("server_db_archive_path").unwrap().path();
// 				crate::podman_fns::podman_pg_dump(&db_container_name, db_name, db_user_name, server_db_archive_path).await;

// 				let candidate_id = propose_candidate_ruleset(None, ruleset_name, &next, &client, server_db_archive_path).await?;
// 				apply_candidate_top(&mut client, server_db_archive_path, &candidate_id, true).await?;
// 			}

// 			Ok(())
// 		}).await??;
// 	}

// 	Ok(())
// }



// intended invalid cases:
// ruleset uses sibling bi-directionally without pivot state
// ruleset uses child
// ruleset uses grand child


// obvious invalid cases:
// invalid db_schema
// invalid db_migration
// invalid ts_code
// invalid db_uses_functions
// invalid db_uses_tables



// pure tests for validate_schema_uses and validate_schema_references

// pure tests for add_and_check_ruleset_grant_commands


// podman_votebase_tsc I guess
