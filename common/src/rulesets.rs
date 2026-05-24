use std::collections::HashMap;

// use db_generated::queries::reflection::GetForeignKeys;

use crate::podman_fns::podman_compute_diff;
use crate::runtime::RuntimeError;
use crate::{
	FnType, PgClient, PgConfig, RoleType, format_full_path, format_ruleset_role, format_ruleset_schema, pg_con, postgres, queries /*format_ruleset_schema*/
};
use crate::db_types::votebase_catalog::{
	RulesetFnBorrowed,
};


// TODO https://github.com/Aleph-Alpha/ts-rs
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BundledRuleset {
	/// Typescript code containing all the Actions and Views of the `Ruleset`.
	/// This must be fully bundled, meaning it's the entire codebase of the whole `Ruleset`, including the queries code etc.
	/// It doesn't need to include the votebase runtime.
	pub ts_code: String,

	/// The final intended database schema.
	/// Used to check that `db_migration` does what it's intended to.
	pub db_schema: String,
	/// The migration intended to actually be run to reach the state of `db_schema`.
	/// This will be checked to ensure it actually goes from the *current* state of the `Ruleset` database to the one declared in `db_schema`.
	pub db_migration: String,

	// /// A mapping of the static children of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	// /// If this `Ruleset` replaces the existing one, this will be the absolute state of the static children, with any existing ones changed to match their new description and extra ones recursively deleted.
	// pub static_children: HashMap<String, KeepOrReplace<BundledRuleset>>,

	// TODO I'm cutting even static events for right this second
	// /// A mapping of the static recurring events of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	// /// If this `Ruleset` replaces the existing one, this will be the absolute state of the static recurring events, with any existing ones changed to match their new description and extra ones deleted.
	// pub static_recurring_events: HashMap<String, KeepOrReplace<StaticRecurringEvent>>,

	// TODO dynamic children and and events is scope I'm cutting for now
	// /**
	//  * A predicate that determines what dynamic children to keep.
	//  * All others will be recursively deleted.
	// */
	// pub dynamic_children_keep_rule: String,
	// /**
	//  * A predicate that determines what dynamic recurring events to keep.
	//  * All others will be deleted.
	// */
	// pub dynamic_recurring_event_keep_rule: String,
	// /**
	//  * A predicate that determines what scheduled events to keep.
	//  * All others will be deleted.
	// */
	// pub dynamic_standalone_event_keep_rule: String,
}

// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub enum KeepOrReplace<T> {
// 	Keep,
// 	Replace(T),
// }

// #[derive(Debug, serde::Deserialize)]
// pub struct StaticRecurringEvent {
// 	pub description: String, pub start: chrono::DateTime<chrono::Utc>,
// 	pub recurrence_granularity: GranularityEnum, pub recurrence_multiplier: u16,
// 	pub action_name: String, pub action_arg: serde_json::Value,
// }


#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
	#[error("the migration for `{0}` doesn't match the provided schema")]
	InvalidMigration(String),
	#[error("a static child was indicated for a slot that doesn't already have one `{0}`")]
	UnspecifiedStaticChild(String),
	#[error("the typescript code has errors\n\n{0}")]
	InvalidTs(String),
	// #[error("the uses of the rulesets have been violated\n\n{0:?}")]
	// InvalidUses(Vec<String>),
	// #[error("there are disallowed foreign keys\n\n{0:?}")]
	// InvalidReferences(Vec<String>),

	#[error(transparent)]
	RuntimeRejected(#[from] RuntimeError),
	#[error(transparent)]
	Container(#[from] utils::temp_containers::ContainerError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Db(#[from] postgres::Error),
}

impl Into<deno_error::JsErrorBox> for ValidationError {
	fn into(self) -> deno_error::JsErrorBox {
		match self {
			ValidationError::InvalidMigration(e) => deno_error::JsErrorBox::generic(e),
			ValidationError::UnspecifiedStaticChild(e) => deno_error::JsErrorBox::generic(e),
			ValidationError::InvalidTs(e) => deno_error::JsErrorBox::generic(e),
			// ValidationError::InvalidUses(e) => deno_error::JsErrorBox::generic(e),
			// ValidationError::InvalidReferences(e) => deno_error::JsErrorBox::generic(e),
			ValidationError::RuntimeRejected(e) => deno_error::JsErrorBox::generic(e.to_string()),
			ValidationError::Container(e) => deno_error::JsErrorBox::generic(e.to_string()),
			ValidationError::Io(e) => deno_error::JsErrorBox::generic(e.to_string()),
			ValidationError::Db(e) => deno_error::JsErrorBox::generic(e.to_string()),
		}
	}
}

#[derive(thiserror::Error, Debug)]
pub enum CandidateApplyError {
	#[error("the candidate {0} couldn't be found")]
	CandidateNotFound(uuid::Uuid),
	// #[error("the candidate ruleset {0} couldn't be read correctly from the database\n\n{1}")]
	// CorruptedCandidate(uuid::Uuid, serde_json::Error),
	#[error("the candidate {0} was found to be invalid for the current state\n\n{1}")]
	Invalid(uuid::Uuid, ValidationError),

	#[error(transparent)]
	Db(#[from] postgres::Error),
}


pub(crate) struct ValidateCtx {
	through_migration_pg: crate::podman_fns::TempNetworkedPg,
	through_schema_pg: crate::podman_fns::TempNetworkedPg,
}

#[derive(Debug)]
pub struct StoredRuleset {
	pub ts_code: String,
	pub db_schema: String,
	// pub static_children: HashMap<String, StoredRuleset>,
	// pub static_recurring_events: HashMap<String, StaticRecurringEvent>,
}

pub async fn podman_votebase_tsc(full_ruleset_dir: &std::path::Path) -> Result<(), ValidationError> {
	let workspace_arg = format!("{}:/workspace/ruleset", full_ruleset_dir.to_string_lossy());
	let output = utils::temp_containers::workspace_podman_run("votebase-tsc", &[], workspace_arg, &["--noEmit"]).await?;

	if !output.status.success() {
		// let all_output = output.stdout.extend(output.stderr);
		let errors = String::from_utf8_lossy(&output.stdout);
		Err(ValidationError::InvalidTs(errors.to_string()))
	}
	else { Ok(()) }
}

pub async fn validate_bundled_ruleset_top(
	parent_full_path: Option<&str>, ruleset_name: &str,
	full_path: &str,
	prev_ruleset: Option<&StoredRuleset>,
	next_bundled_ruleset: &BundledRuleset,
) -> Result<(), ValidationError> {
	let random_suffix = utils::temp_containers::random_string(20);
	let network_name = format!("temp_postgres_network_{random_suffix}");
	let podman_network = utils::temp_containers::PodmanNetwork::new(network_name).await?;

	let through_migration_db_name = "tempdb|through_migration".to_string();
	let through_schema_db_name = "tempdb|through_schema".to_string();
	let (through_migration_pg, through_schema_pg) = tokio::try_join!(
		crate::podman_fns::spawn_networked_votebase_postgres(through_migration_db_name, &podman_network),
		crate::podman_fns::spawn_networked_votebase_postgres(through_schema_db_name, &podman_network),
	)?;

	let ctx = ValidateCtx { through_migration_pg, through_schema_pg };
	validate_bundled_ruleset(full_path, parent_full_path, ruleset_name, &ctx, prev_ruleset, next_bundled_ruleset).await?;
	Ok(())
}

pub(crate) async fn validate_bundled_ruleset(
	full_path: &str,
	parent_full_path: Option<&str>, ruleset_name: &str,
	ctx: &ValidateCtx,
	prev_ruleset: Option<&StoredRuleset>,
	next_bundled_ruleset: &BundledRuleset,
) -> Result<(), ValidationError> {
	// place the typescript code in a temp directory and check it
	let temp_dir = tmpdir::TmpDir::new("validate_bundled_ruleset").await?;
	let ts_file = temp_dir.as_ref().join("ruleset.ts");
	tokio::fs::write(&ts_file, &next_bundled_ruleset.ts_code).await?;
	podman_votebase_tsc(temp_dir.as_ref()).await?;

	// ensure the runtime is okay with it
	let runtime = crate::runtime::Runtime::new(&next_bundled_ruleset.ts_code).await?;

	// every ruleset goes into the table as abstract but with declarations of uses. when we check we always stub. we use the hash of the variable name to help name the stubs
	// this means we only have to figure out the joining of rulesets etc, and perform migration consistency checks on the actual ruleset change as we actually encounter it
	// we'll do all of the ruleset creation within the context of a transaction, which means we can roll it back to wipe the slate clean
	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);
	let formatted_ruleset_schema = format_ruleset_schema(&full_path);

	// TODO this doesn't make sense
	let db_name = "fake_db_name";
	let create_sql = format!(include_str!("./create-ruleset.sql"),
		db_name=db_name,
		formatted_ruleset_schema=formatted_ruleset_schema,
		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
		formatted_ruleset_role_action=formatted_ruleset_role_action,
		formatted_ruleset_role_view=formatted_ruleset_role_view,
	);

	// for through_schema always apply bundled.db_schema
	ctx.through_schema_server_client.batch_execute(&format!(r#"
		set role "votebase_server_{db_name}";
		{create_sql};

		set role "{formatted_ruleset_role_migrator}";
		set search_path to "{formatted_ruleset_schema}";
		{db_schema}
		reset role;
	"#, db_schema=next_bundled_ruleset.db_schema)).await?;

	if let Some(prev_ruleset) = prev_ruleset {
		// for through_migration apply prev.db_schema and then bundled.db_migration
		ctx.through_migration_server_client.batch_execute(&format!(r#"
			set role "votebase_server_{db_name}";
			{create_sql};

			set role "{formatted_ruleset_role_migrator}";
			set search_path to "{formatted_ruleset_schema}";
			{db_schema}
			{db_migration}
			reset role;
		"#, db_schema=prev_ruleset.db_schema, db_migration=next_bundled_ruleset.db_migration)).await?;
	}
	else {
		// there *isn't* an old schema, so just do the same
		ctx.through_migration_server_client.batch_execute(&format!(r#"
			set role "votebase_server_{db_name}";
			{create_sql};

			set role "{formatted_ruleset_role_migrator}";
			set search_path to "{formatted_ruleset_schema}";
			{db_schema}
			reset role;
		"#, db_schema=next_bundled_ruleset.db_schema)).await?;
	}

	// in the limit we can't actually just diff things immediately,
	// since we need to do all the children that we may point at
	// and the rest of the entire ruleset tree outside of us that we may point at
	// this means we actually have to do this in validate_bundled_ruleset_top???
	// TODO but for now we're doing it naive and just doing it immediately
	let diff = podman_compute_diff(
		&ctx.db_container_name, &formatted_ruleset_schema,
		&ctx.through_migration_config, &ctx.through_schema_config,
	).await?;
	if !diff.is_empty() {
		return Err(ValidationError::InvalidMigration(full_path.to_string()))
	}

	// Box::pin(
	// 	validate_bundled_ruleset_children(
	// 		full_path, &ctx,
	// 		prev_ruleset.map(|r| &r.static_children), &next_bundled_ruleset.static_children,
	// 	)
	// ).await?;

	Ok(())
}

// async fn validate_bundled_ruleset_children(
// 	parent_full_path: &str,
// 	ctx: &ValidateCtx,
// 	prev_ruleset_children: Option<&HashMap<String, StoredRuleset>>,
// 	next_ruleset_children: &HashMap<String, KeepOrReplace<BundledRuleset>>,
// ) -> Result<(), ValidationError> {
// 	match prev_ruleset_children {
// 		// there were no children in the previous ruleset, or the previous ruleset didn't exist
// 		None => {
// 			for (child_ruleset_name, child_ruleset) in next_ruleset_children {
// 				if let KeepOrReplace::Replace(child_ruleset) = child_ruleset {
// 					let full_path = crate::format_full_path(Some(parent_full_path), child_ruleset_name);
// 					validate_bundled_ruleset(&full_path, Some(parent_full_path), child_ruleset_name, ctx, None, child_ruleset).await?;
// 				}
// 				else {
// 					return Err(ValidationError::UnspecifiedStaticChild(child_ruleset_name.to_owned()))
// 				}
// 			}

// 			Ok(())
// 		},
// 		// there were some children in the previous ruleset
// 		Some(prev_ruleset_children) => {
// 			use crate::{outer_join, JoinOption};
// 			let joined_children = outer_join(prev_ruleset_children, next_ruleset_children);
// 			for (child_ruleset_name, join_option) in joined_children {
// 				let full_path = crate::format_full_path(Some(parent_full_path), child_ruleset_name);

// 				match join_option {
// 					JoinOption::Left(prev_ruleset) => {
// 						recursively_delete_ruleset(&full_path, prev_ruleset, &ctx.reset_server_client).await?;
// 						recursively_delete_ruleset(&full_path, prev_ruleset, &ctx.migrate_server_client).await?;
// 					},

// 					JoinOption::Right(KeepOrReplace::Replace(next_ruleset)) => {
// 						validate_bundled_ruleset(&full_path, Some(parent_full_path), child_ruleset_name, ctx, None, next_ruleset).await?;
// 					},
// 					JoinOption::Right(KeepOrReplace::Keep) => {
// 						return Err(ValidationError::UnspecifiedStaticChild(child_ruleset_name.to_owned()))
// 					},

// 					JoinOption::Both(prev_ruleset, KeepOrReplace::Replace(next_ruleset)) => {
// 						validate_bundled_ruleset(&full_path, Some(parent_full_path), child_ruleset_name, ctx, Some(prev_ruleset), next_ruleset).await?;
// 					},
// 					JoinOption::Both(_prev_ruleset, KeepOrReplace::Keep) => {
// 						// do nothing! this branch of the tree remains as is
// 					},
// 				}
// 			}

// 			Ok(())
// 		},
// 	}
// }

// fn validate_schema_uses(
// 	possibly_effected_uses: &Vec<(String, Vec<ConcreteFunctionUse>, Vec<ConcreteTableUse>)>,
// 	existent_objects_by_ruleset_path_and_name: &hashbrown::HashSet<UseableObject>,
// ) -> Result<(), Vec<String>> {
// 	let mut errors = vec![];

// 	for (using_ruleset_path, function_uses, table_uses) in possibly_effected_uses {

// 		for ConcreteTableUse {
// 			ruleset_path: used_ruleset_path, object_name,
// 			columns: used_columns,
// 		} in table_uses {

// 			if used_ruleset_path.starts_with(using_ruleset_path) {
// 				errors.push(format!(
// 					"ruleset {using_ruleset_path} is attempting to use {used_ruleset_path}.{object_name}, but rulesets aren't allowed to reference their descendants",
// 				));
// 				continue
// 			}

// 			let obj = existent_objects_by_ruleset_path_and_name.get(&(used_ruleset_path.as_str(), object_name.as_str()));
// 			if let Some(UseableObject { db_object: UseableDbObject::Table { columns: usable_columns }, .. }) = obj {
// 				for UseColumn { name: use_col_name, pg_type: use_col_pg_type_name, can_null, use_kind } in used_columns {
// 					match usable_columns.iter().find(|c| &c.name == use_col_name) {
// 						Some(usable_col) => {
// 							if &usable_col.pg_type_name != use_col_pg_type_name {
// 								errors.push(format!(
// 									"ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name}.{use_col_name} as type {use_col_pg_type_name}, but available type is {}",
// 									usable_col.pg_type_name
// 								));
// 							}
// 							if usable_col.not_null && *can_null {
// 								errors.push(format!(
// 									"ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name}.{use_col_name} expecting not null, but it's nullable",
// 								));
// 							}
// 							match (use_kind, &usable_col.allowed_use) {
// 								(_, ColumnUseKind::Both) => { /* everything is allowed */ },
// 								(ColumnUseKind::Both | ColumnUseKind::Query, ColumnUseKind::Reference) => {
// 									errors.push(format!(
// 										"ruleset {using_ruleset_path} tries to use {used_ruleset_path}.{object_name}.{use_col_name} to query, which isn't allowed",
// 									));
// 								},
// 								(ColumnUseKind::Both | ColumnUseKind::Reference, ColumnUseKind::Query) => {
// 									errors.push(format!(
// 										"ruleset {using_ruleset_path} tries to use {used_ruleset_path}.{object_name}.{use_col_name} to reference, which isn't allowed",
// 									));
// 								},
// 								_ => {}
// 							}
// 						},
// 						None => {
// 							errors.push(format!(
// 								"ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name}.{use_col_name}, but that column is not available"
// 							));
// 						}
// 					}
// 				}
// 			}
// 			else if let Some(UseableObject { db_object: UseableDbObject::Function { .. }, .. }) = obj {
// 				errors.push(format!("ruleset {using_ruleset_path} tries to use {used_ruleset_path}.{object_name} as a table, but it's a function"));
// 			}
// 			else {
// 				errors.push(format!("ruleset {using_ruleset_path} tries to use {used_ruleset_path}.{object_name}, but it doesn't exist"));
// 			}
// 		}

// 		for ConcreteFunctionUse {
// 			ruleset_path: used_ruleset_path, object_name,
// 			is_action: used_is_action, params: used_params, return_type: used_return_type,
// 		} in function_uses {

// 			if used_ruleset_path.starts_with(using_ruleset_path) {
// 				errors.push(format!(
// 					"ruleset {using_ruleset_path} is attempting to use {used_ruleset_path}.{object_name}, but rulesets aren't allowed to reference their descendants",
// 				));
// 				continue
// 			}

// 			let obj = existent_objects_by_ruleset_path_and_name.get(&(used_ruleset_path.as_str(), object_name.as_str()));
// 			if let Some(UseableObject { db_object: UseableDbObject::Function { is_action: usable_is_action, function_params: usable_params, return_type: usable_return_type }, .. }) = obj {
// 				if *used_is_action && !usable_is_action {
// 					errors.push(format!("ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name} as volatile, but that isn't allowed"));
// 				}
// 				if used_return_type != usable_return_type {
// 					errors.push(format!("ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name} with return type {used_return_type}, but it returns {usable_return_type}"));
// 				}

// 				if used_params.len() != usable_params.len() {
// 					errors.push(format!(
// 						"ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name} with {} params, but it has {} params",
// 						used_params.len(),
// 						usable_params.len(),
// 					));
// 					continue
// 				}
// 				for (i, (use_param_type, usable_param)) in used_params.iter().zip(usable_params.iter()).enumerate() {
// 					if use_param_type != usable_param {
// 						errors.push(format!(
// 							"ruleset {using_ruleset_path} uses {used_ruleset_path}.{object_name} param {} as type {}, but available type is {}",
// 							i, use_param_type, usable_param,
// 						));
// 					}
// 				}
// 			}
// 			else if let Some(UseableObject { db_object: UseableDbObject::Table { .. }, .. }) = obj {
// 				errors.push(format!("ruleset {using_ruleset_path} tries to use {used_ruleset_path}.{object_name} as a function, but it's a table"));
// 			}
// 			else {
// 				errors.push(format!("ruleset {using_ruleset_path} tries to use {used_ruleset_path}.{object_name}, but it doesn't exist"));
// 			}
// 		}
// 	}

// 	if errors.len() > 0 { Err(errors) }
// 	else { Ok(()) }
// }

// fn validate_schema_references(
// 	foreign_key_references: &Vec<GetForeignKeys>,
// 	table_references_by_ruleset_and_table_and_column: &HashMap<(&str, &str, &str), UseColumn>,
// ) -> Result<(), Vec<String>> {
// 	let mut errors = vec![];

// 	for GetForeignKeys { using_ruleset_path, used_ruleset_path, table_name, column_name } in foreign_key_references {
// 		let table_use = table_references_by_ruleset_and_table_and_column.get(&(&used_ruleset_path, &table_name, &column_name));

// 		if let Some(UseColumn { use_kind: ColumnUseKind::Reference | ColumnUseKind::Both, .. }) = table_use {
// 			/* we're good, the key is justified by a reference use */
// 		}
// 		else if let Some(UseColumn { use_kind: ColumnUseKind::Query, .. }) = table_use {
// 			errors.push(format!(
// 				"ruleset {using_ruleset_path} still has a foreign key pointing to {used_ruleset_path}.{table_name}.{column_name}, but its declared \"use\" only allows it to query that column, not reference it",
// 			));
// 		}
// 		else {
// 			errors.push(format!(
// 				"ruleset {using_ruleset_path} still has a foreign key pointing to {used_ruleset_path}.{table_name}.{column_name}, but it doesn't have a declared \"use\" to reference that column",
// 			));
// 		}
// 	}

// 	if errors.len() > 0 { Err(errors) }
// 	else { Ok(()) }
// }

// async fn add_and_check_ruleset_grant_commands_top(
// 	server_client: &PgClient,
// 	grant_commands: &mut Vec<String>,
// 	possibly_effected_uses: &Vec<(String, Vec<ConcreteFunctionUse>, Vec<ConcreteTableUse>)>,
// ) -> Result<(), ValidationError> {

// 	let function_grants = queries::reflection::get_function_grants();
// 	let column_grants = queries::reflection::get_column_grants();
// 	let (function_grants, column_grants) = futures::try_join!(
// 		function_grants.bind(server_client).all(),
// 		column_grants.bind(server_client).all(),
// 	)?;

// 	add_and_check_ruleset_grant_commands(grant_commands, possibly_effected_uses, function_grants, column_grants)?;

// 	Ok(())
// }

// use crate::queries::reflection::{GetColumnGrants, GetFunctionGrants};





// fn add_and_check_ruleset_grant_commands(
// 	grant_commands: &mut Vec<String>,
// 	actual_function_uses: &HashMap<(&str, &str), ConcreteFunctionUse>,
// 	actual_column_uses: &HashMap<(&str, &str, &str), ConcreteTableUse>,
// 	function_grants: Vec<GetFunctionGrants>,
// 	column_grants: Vec<GetColumnGrants>,
// ) -> Result<(), ValidationError> {
// 	// essentially this needs to iterate through the *grants*, the ones that actually exist, and for each try to determine whether it *should* exist
// 	// if it shouldn't, we just revoke it, assuming that it previously had a valid uses but the object was renamed or something (and we don't have the ability to tell if such renames etc happened, at least without writing a postgres static analysis engine)
// 	// the only thing we actually flag as a *problem* is if there's a reference still in existence that is no longer justified by a uses, since that means that ruleset itself didn't clean it up. that's something we need to fail the validation for

// 	// we could also just revoke *all* existing grants and then issue commands to grant all the new ones at the end?
// 	// revoking a grant is cheap and won't error as long as the objects it's referencing exist, which here we know

// 	// here's a problem, we need to not only check the *grants* to see if they're justified, we also need to check *all* the foreign keys held by a ruleset to ensure it's justified
// 	// this ensures we catch ones that never made any sense

// 	// the easiest way to do this is just to revoke everything for all the rulesets a ruleset actually points to
// 	// although we still have figure out which ones it actually points to in order to not waste a huge amount of time
// 	// revoke all privileges on all tables in schema schema_name from role_name;
// 	// revoke all privileges on all sequences in schema schema_name from role_name;
// 	// revoke all privileges on all functions in schema schema_name from role_name;
// 	// revoke all privileges on all routines in schema schema_name from role_name;
// 	// revoke all privileges on schema schema_name from role_name;


// 	for GetFunctionGrants {
// 		 using_ruleset_path, used_ruleset_path, function_name, function_params, has_execute_priv,
// 	} in function_grants {
// 		// find out if this function grant is justified by a uses
// 		if let Some(function_use) = actual_function_uses.get(&(&using_schema, &function_name)) {
// 			// we need to be sure the types of the granted object actually correspond to the types of the use

// 		}
// 	}

// 	for GetColumnGrants {
// 		using_ruleset_path, used_ruleset_path, table_name, column_name, has_reference_priv, has_select_priv,
// 	} in column_grants {
// 		// find out if this column grant is justified by a uses
// 		let column_use = actual_column_uses.get(&(&using_schema, &table_name, &column_name));
// 	}

// 	Ok(())
// }

// enum GrantKind { Granting, Revoking }
// fn add_function_grant_commands(
// 	grant_commands: &mut Vec<String>, granting: GrantKind,
// 	formatted_used_schema: &str, object_name: &str, is_action: bool,
// 	formatted_using_role_action: &str, formatted_using_role_view: &str,
// ) {
// 	// TODO should we allow the migrator role to call functions in foreign rulesets? I don't think we should
// 	if let GrantKind::Granting = granting {
// 		grant_commands.push(format!(r#"
// 			grant all permissions on function "{formatted_used_schema}"."{object_name}" to "{formatted_using_role_action}";
// 		"#));

// 		if !is_action { grant_commands.push(format!(r#"
// 			grant all permissions on function "{formatted_used_schema}"."{object_name}" to "{formatted_using_role_view}";
// 		"#)); }
// 	}
// 	else {
// 		grant_commands.push(format!(r#"
// 			revoke all permissions on function "{formatted_used_schema}"."{object_name}" from "{formatted_using_role_action}";
// 		"#));

// 		if !is_action { grant_commands.push(format!(r#"
// 			revoke all permissions on function "{formatted_used_schema}"."{object_name}" from "{formatted_using_role_view}";
// 		"#)); }
// 	}
// }


pub async fn create_ruleset(
	db_name: &str, client: &mut PgClient,
	parent_full_path: Option<&str>, name: &str,
	bundled_ruleset: &BundledRuleset,
	fns: &Vec<(String, FnType)>
	// function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
	// table_uses: &'u Vec<(&'u str, &'u str, Vec<DbUsesColumnStructBorrowed<'u>>)>,
) -> Result<(), postgres::Error> {

	let BundledRuleset { ts_code, db_schema, db_migration } = bundled_ruleset;
	let fns = db_generated::IterSql(|| fns.iter()
		.map(|(name, fn_type)| RulesetFnBorrowed { name: name.as_str(), fn_type: (*fn_type).into() }));

	// let function_uses = IterSql(||
	// 	function_uses.iter().map(|(ruleset_path, object_name, is_action, return_type, params)|
	// 		DbUsesFunctionStructParams {
	// 			ruleset_path, object_name, is_action: *is_action, param_types: params.as_slice(), return_type,
	// 		}
	// 	)
	// );
	// let table_uses = IterSql(||
	// 	table_uses.iter().map(|(ruleset_path, object_name, columns)|
	// 		DbUsesTableStructParams {
	// 			ruleset_path, object_name, columns: columns.as_slice(),
	// 		}
	// 	)
	// );

	let full_path = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &ts_code, &db_schema, &fns)
		.one().await?;

	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);
	let formatted_ruleset_schema = format_ruleset_schema(&full_path);

	let mut transaction = client.transaction().await?;
	let create_sql = format!(include_str!("./create-ruleset.sql"),
		db_name=db_name,
		formatted_ruleset_schema=formatted_ruleset_schema,
		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
		formatted_ruleset_role_action=formatted_ruleset_role_action,
		formatted_ruleset_role_view=formatted_ruleset_role_view,
	);
	transaction.batch_execute(&create_sql).await?;

	// TODO same role concerns
	// TODO also the alter role stuff doesn't count because postgres only counts when you connect as that role ugh
	sql_as_role(&mut transaction, &formatted_ruleset_schema, &formatted_ruleset_role_migrator, db_schema).await?;
	transaction.commit().await?;

	Ok(())
}

async fn sql_as_role(
	transaction: &mut postgres::Transaction<'_>,
	formatted_ruleset_schema: &str,
	formatted_ruleset_role: &str,
	sql: &str,
) -> Result<(), postgres::Error> {
	transaction.batch_execute(&format!(r#"
		set role "{formatted_ruleset_role}";
		set search_path to "{formatted_ruleset_schema}";
		{sql}
		reset role;
	"#)).await
}

// async fn create_ruleset_validation<'u>(
// 	base_config: &PgConfig, client: &PgClient,
// 	parent_full_path: Option<&str>, name: &str,
// 	ruleset_code: &str, db_schema: &str, fns: &Vec<(String, FnType)>,
// 	// function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
// 	// table_uses: &'u Vec<(&'u str, &'u str, Vec<DbUsesColumnStructBorrowed<'u>>)>,
// ) -> Result<(PgClient, String), postgres::Error> {

// 	use db_generated::IterSql;
// 	let fns = IterSql(|| fns.iter().map(|(name, fn_type)| RulesetFnBorrowed { name, fn_type: *fn_type }));
// 	// let function_uses = IterSql(||
// 	// 	function_uses.iter().map(|(ruleset_path, object_name, is_action, return_type, params)|
// 	// 		DbUsesFunctionStructParams {
// 	// 			ruleset_path, object_name, is_action: *is_action, param_types: params.as_slice(), return_type,
// 	// 		}
// 	// 	)
// 	// );
// 	// let table_uses = IterSql(||
// 	// 	table_uses.iter().map(|(ruleset_path, object_name, columns)|
// 	// 		DbUsesTableStructParams {
// 	// 			ruleset_path, object_name, columns: columns.as_slice(),
// 	// 		}
// 	// 	)
// 	// );

// 	let full_path = queries::rulesets::insert_ruleset()
// 		.bind(client, &parent_full_path, &name, &ruleset_code, &db_schema, &(&[] as &[RulesetFnBorrowed<'static>]))
// 		.one().await?;

// 	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
// 	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
// 	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

// 	let formatted_ruleset_schema = format_ruleset_schema(&full_path);
// 	let create_sql = format!(include_str!("./create-ruleset.sql"),
// 		formatted_ruleset_schema=formatted_ruleset_schema,
// 		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
// 		formatted_ruleset_role_action=formatted_ruleset_role_action,
// 		formatted_ruleset_role_view=formatted_ruleset_role_view,
// 	);
// 	client.batch_execute(&create_sql).await?;

// 	let mut migrator_config = base_config.clone();
// 	migrator_config.user(formatted_ruleset_role_migrator);
// 	let migrator_client = pg_con(&migrator_config).await?;
// 	migrator_client.batch_execute(db_schema).await?;

// 	Ok((migrator_client, formatted_ruleset_schema))
// }


pub async fn apply_candidate_top(
	server_role_client: &mut PgClient,
	// server_db_archive_path: &std::path::Path,
	candidate_id: &uuid::Uuid,
	delete_other_candidates: bool,
) -> Result<(), CandidateApplyError> {
	let (candidate_for, bundled_ruleset, fns) = queries::rulesets::get_ruleset_candidate().bind(server_role_client, candidate_id)
		.map(|r| {
			let bundled_ruleset = BundledRuleset {
				ts_code: r.ts_code.into(),
				db_schema: r.db_schema.into(),
				db_migration: r.db_migration.into(),
			};
			let fns: Vec<(String, FnType)> = r.fns.map(|f| (f.name.into(), f.fn_type.into())).collect();
			(r.candidate_for.to_string(), bundled_ruleset, fns)
		})
		.opt().await?
		.ok_or_else(|| CandidateApplyError::CandidateNotFound(*candidate_id))?;
		// .map_err(|e| CandidateApplyError::CorruptedCandidate(*candidate_id, e))?;

	let prev_ruleset = get_stored_ruleset(&server_role_client, &candidate_for).await?;
	let (parent_full_path, ruleset_name) = crate::split_full_path(&candidate_for);

	validate_bundled_ruleset_top(
		server_role_client,
		parent_full_path.as_deref(), &ruleset_name, &candidate_for,
		Some(&prev_ruleset), &bundled_ruleset, /*server_db_archive_path,*/
	).await.map_err(|e| CandidateApplyError::Invalid(*candidate_id, e))?;

	// let prev_has_candidate_references = prev_ruleset.db_uses_tables.iter().any(|t| {
	// 	t.ruleset_path == "votebase_catalog" && t.object_name == "candidate_replacement_ruleset"
	// 	&& t.columns.iter().any(|c| c.use_kind == ColumnUseKind::Reference)
	// });

	// we always have *at least* a root ruleset, so whenever apply_candidate_top is being called it's to replace a ruleset
	let mut server_role_tx = server_role_client.transaction().await?;
	apply_candidate(
		&mut server_role_tx,
		parent_full_path.as_deref(), &ruleset_name, &candidate_for, Some(prev_ruleset), bundled_ruleset,
		&fns,
	).await?;

	// we always delete other candidates if the old ruleset can't possibly be referencing any of them
	// (which means it can't or at least shouldn't be using them in any continuous decision processes)
	if delete_other_candidates /*|| !prev_has_candidate_references*/ {
		queries::rulesets::delete_candidate_and_others().bind(&mut server_role_tx, candidate_id).await?;
	}
	else {
		queries::rulesets::delete_candidate_only().bind(&mut server_role_tx, candidate_id).await?;
		// TODO validate any *remaining* candidates to see if they're actually valid to apply to the new state!
		// so go through the existing candidates, and for any that fails validation just delete it
		// validate_bundled_ruleset_top(
		// 	parent_full_path.as_deref(), &ruleset_name, &candidate_for,
		// 	Some(&prev_ruleset), &bundled_ruleset, server_db_archive_path,
		// ).await.map_err(|e| CandidateApplyError::Invalid(*candidate_id, e))?;
	}

	server_role_tx.commit().await?;

	Ok(())
}

pub(crate) async fn apply_candidate(
	server_role_tx: &mut postgres::Transaction<'_>,
	parent_full_path: Option<&str>, ruleset_name: &str, full_path: &str,
	prev_ruleset: Option<StoredRuleset>,
	bundled_ruleset: BundledRuleset,
	fns: &Vec<(String, FnType)>,
) -> Result<(), CandidateApplyError> {
	let BundledRuleset { ts_code, db_schema, db_migration, /*db_uses_functions, db_uses_tables, static_children*/ } = bundled_ruleset;
	let fns = db_generated::IterSql(|| fns.iter()
		.map(|(name, fn_type)| RulesetFnBorrowed { name: name.as_str(), fn_type: (*fn_type).into() }));

	let formatted_using_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	// let formatted_using_role_action = format_ruleset_role(&full_path, RoleType::Action);
	// let formatted_using_role_view = format_ruleset_role(&full_path, RoleType::View);
	let formatted_ruleset_schema = format_ruleset_schema(full_path);

	// either migrate and update the ruleset if there was one previously...
	if let Some(_prev_ruleset) = prev_ruleset {
		sql_as_role(server_role_tx, &formatted_ruleset_schema, &formatted_using_role_migrator, &db_migration).await?;

		// remove all grants the prev_ruleset had to foreign rulesets, and (re)grant all the new ones
		// TODO in order to grant all the new ones, all the things you're granting to have to exist! this is fine if children only reference ancestors, but not fine if people start doing sibling or cousin etc references
		// so before we do all the grants, we need to construct the entire new tree? that doesn't work cleanly because references grants are required in order for someone else to possibly construct a new foreign key
		// so we have no real choice but to build the *siblings* in dag order (even more complicated if we're doing cousin references)
		// this is at least necessary for *reference* uses, at least ones that will newly be exercised by creating references
		// all the other ones though
		// let mut grant_commands = vec![];
		// add_and_check_ruleset_grant_commands_top(server_client, grant_commands, possibly_effected_uses).await?;

		let _rows_updated = queries::rulesets::update_ruleset()
			.bind(server_role_tx, &ts_code, &db_schema, &fns, &full_path).await?;
	}
	// ... or create it from scratch
	// TODO only relevant when there are children!
	else {
		// let db_uses_functions: Vec<_> = db_uses_functions.iter().map(|u| {
		// 	let params_vec: Vec<&str> = u.params.iter().map(AsRef::as_ref).collect();
		// 	(u.ruleset_path.as_str(), u.object_name.as_str(), u.is_action, u.return_type.as_str(), params_vec)
		// }).collect();

		// let db_uses_tables: Vec<_> = db_uses_tables.iter().map(|u| {
		// 	let columns_vec: Vec<DbUsesColumnStructBorrowed> = u.columns.iter().map(|c| DbUsesColumnStructBorrowed {
		// 		name: &c.name,
		// 		typ: &c.pg_type,
		// 		can_null: c.can_null,
		// 		use_kind: c.use_kind.into_db(),
		// 	}).collect();
		// 	(u.ruleset_path.as_str(), u.object_name.as_str(), columns_vec)
		// }).collect();

		// create_ruleset(
		// 	db_name, &mut client, parent_full_path, ruleset_name, &ts_code, &db_schema, fns,
		// ).await?;

		unimplemented!();
	}

	// apply_candidate_children(parent_full_path, prev_ruleset_children, next_ruleset_children).await?;

	// TODO allow "exposes" on only these two votebase_catalog items
	// grant usage on schema votebase_catalog to "{formatted_ruleset_role_view}";
	// grant usage on schema votebase_catalog to "{formatted_ruleset_role_action}";
	// grant usage on schema votebase_catalog to "{formatted_ruleset_role_migrator}";

	// grant references (id) on table votebase_catalog.member to "{formatted_ruleset_role_migrator}";
	// grant references (id) on table votebase_catalog.candidate_replacement_ruleset to "{formatted_ruleset_role_migrator}";

	// grant select(full_path, parent_full_path, "name", ts_code, db_schema, fns, db_uses_functions, db_uses_tables) on table votebase_catalog.ruleset to "{formatted_ruleset_role_action}";
	// grant select(full_path, parent_full_path, "name", ts_code, db_schema, fns, db_uses_functions, db_uses_tables) on table votebase_catalog.ruleset to "{formatted_ruleset_role_view}";

	// grant select on table votebase_catalog.candidate_replacement_ruleset to "{formatted_ruleset_role_view}";
	// grant select on table votebase_catalog.candidate_replacement_ruleset to "{formatted_ruleset_role_action}";
	Ok(())
}

// async fn apply_candidate_children(
// 	parent_full_path: &str,
// 	prev_ruleset_children: Option<&HashMap<String, StoredRuleset>>,
// 	next_ruleset_children: &HashMap<String, KeepOrReplace<BundledRuleset>>,
// ) -> Result<(), CandidateApplyError> {
// 	unimplemented!()
// }

async fn get_stored_ruleset(client: &PgClient, full_path: &str) -> Result<StoredRuleset, postgres::Error> {
	queries::rulesets::get_stored_ruleset().bind(client, &full_path)
		.map(|r| StoredRuleset { ts_code: r.ts_code.into(), db_schema: r.db_schema.into() })
		.one().await

	// use std::collections::HashMap;
	// fn build_ruleset_tree(mut rows: Vec<RulesetRow>) -> StoredRuleset {
	//  // PANIC SAFETY, relies on the sql having sorted all the items by their full_path length, and on the query having returned results
	// 	let root_row = rows.swap_remove(0);
	// 	let mut root = StoredRuleset {
	// 		ts_code: root_row.ts_code,
	// 		db_schema: root_row.db_schema,
	// 		static_children: HashMap::new(),
	// 	};

	// 	attach_children(&mut root, &root_row.full_path, &mut rows);

	// 	root
	// }

	// fn attach_children(parent: &mut StoredRuleset, parent_path: &str, rows: &mut Vec<RulesetRow>) {
	// 	let mut i = 0;

	// 	while i < rows.len() {
	// 		let is_child = &rows[i].parent_full_path.unwrap_default() == parent_path.as_str();
	// 		if !is_child {
	// 			i += 1;
	// 			continue;
	// 		}

	// 		let child_row = rows.swap_remove(i);
	// 		let mut child_struct = StoredRuleset {
	// 			ts_code: child_row.ts_code,
	// 			db_schema: child_row.db_schema,
	// 			static_children: HashMap::new(),
	// 		};

	// 		attach_children(&mut child_struct, &child_row.full_path, rows);

	// 		parent.static_children.insert(child_row.name, child_struct);
	// 	}
	// }
}


// // TODO is this necessary given the on delete cascade on parent_full_path?
// // honestly a big part of me wants to *get rid* of that delete cascade, and do these deletes in a bottom up way
// // the big tradeoff is between control of the process, the opportunity to do other work when you delete a ruleset:
// // vs having it be automatic
// async fn recursively_delete_ruleset(
// 	full_path: &str,
// 	ruleset: &StoredRuleset,
// 	server_client: &PgClient,
// ) -> Result<(), postgres::Error> {
// 	queries::rulesets::delete_ruleset().bind(server_client, &full_path).await?;

// 	for (child_name, child_ruleset) in &ruleset.static_children {
// 		let child_full_path = crate::format_full_path(Some(full_path), &child_name);

// 		Box::pin(
// 			recursively_delete_ruleset(&child_full_path, &child_ruleset, server_client)
// 		).await?;
// 	}


// 	Ok(())
// }

pub async fn propose_candidate_ruleset(
	full_path: &str, parent_full_path: Option<&str>, ruleset_name: &str,
	candidate: &BundledRuleset,
	server_role_client: &PgClient,
	// server_db_archive_path: &std::path::Path,
) -> Result<uuid::Uuid, ValidationError> {
	let prev_ruleset = get_stored_ruleset(&server_role_client, full_path).await?;

	let fns = validate_bundled_ruleset_top(
		server_role_client,
		parent_full_path, ruleset_name, full_path,
		Some(&prev_ruleset), candidate,
		// server_db_archive_path,
	).await?;

	let fns = db_generated::IterSql(|| fns.iter()
		.map(|(name, fn_type)| RulesetFnBorrowed { name: name.as_str(), fn_type: (*fn_type).into() }));

	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(server_role_client, &full_path, &candidate.ts_code, &candidate.db_schema, &candidate.db_migration, &fns)
		.one().await?;

	Ok(candidate_uuid)
}
