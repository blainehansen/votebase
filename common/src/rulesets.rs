use std::collections::HashMap;

use db_generated::queries::reflection::GetForeignKeys;

use crate::podman_fns::{podman_compute_diff, podman_pg_restore};
use crate::runtime::RuntimeError;
use crate::{
	PgClient, PgConfig, RoleType, format_full_path, format_ruleset_role, format_ruleset_schema, pg_con, postgres, queries /*format_ruleset_schema*/
};
use crate::db_types::votebase_catalog::{
	FnType, RulesetFnRawBorrowed,
	DbUsesFunctionStructParams, DbUsesTableStructParams, DbUsesColumnStructBorrowed,
};


// TODO https://github.com/Aleph-Alpha/ts-rs
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BundledRuleset {
	/// Typescript code containing all the Actions and Views of the `Ruleset`.
	/// This must be fully bundled, meaning it's the entire codebase of the whole `Ruleset`, including the queries code etc.
	/// It doesn't need to include the votebase runtime.
	pub ts_code: String,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// pub fns: { [fn_name: string]: VotebaseFn<JsonValue> },
	// is there a world where the fns are all declared separately, and then some "shared code" chunk also? how to do this? create temp files for each to do all the checking?

	/// The final intended database schema.
	/// Used to check that `db_migration` does what it's intended to.
	pub db_schema: String,
	/// The migration intended to actually be run to reach the state of `db_schema`.
	/// This will be checked to ensure it actually goes from the *current* state of the `Ruleset` database to the one declared in `db_schema`.
	pub db_migration: String,
	/// The fully qualified names and nature of all the database functions this `Ruleset` uses as its `requires`.
	pub db_uses_functions: Vec<ConcreteFunctionUse>,
	/// The fully qualified names and nature of all the database tables this `Ruleset` uses as its `requires`.
	pub db_uses_tables: Vec<ConcreteTableUse>,

	/// A mapping of the static children of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	/// If this `Ruleset` replaces the existing one, this will be the absolute state of the static children, with any existing ones changed to match their new description and extra ones recursively deleted.
	pub static_children: HashMap<String, KeepOrReplace<BundledRuleset>>,

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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum KeepOrReplace<T> {
	Keep,
	Replace(T),
}

// #[derive(Debug, serde::Deserialize)]
// pub struct StaticRecurringEvent {
// 	pub description: String, pub start: chrono::DateTime<chrono::Utc>,
// 	pub recurrence_granularity: GranularityEnum, pub recurrence_multiplier: u16,
// 	pub action_name: String, pub action_arg: serde_json::Value,
// }


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConcreteFunctionUse {
	pub ruleset_path: String,
	pub object_name: String,
	pub is_action: bool,
	pub params: Vec<String>,
	pub return_type: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConcreteTableUse {
	pub ruleset_path: String,
	pub object_name: String,
	pub columns: Vec<UseColumn>,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct UseColumn {
	pub name: String,
	pub pg_type: String,
	pub can_null: bool,
	pub use_kind: ColumnUseKind,
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Hash)]
pub enum ColumnUseKind { Query, Reference, Both }
impl From<crate::db_types::votebase_catalog::ColumnUseKind> for ColumnUseKind {
	fn from(value: crate::db_types::votebase_catalog::ColumnUseKind) -> Self {
		match value {
			crate::db_types::votebase_catalog::ColumnUseKind::Query => ColumnUseKind::Query,
			crate::db_types::votebase_catalog::ColumnUseKind::Reference => ColumnUseKind::Reference,
			crate::db_types::votebase_catalog::ColumnUseKind::Both => ColumnUseKind::Both,
		}
	}
}
impl ColumnUseKind {
	fn into_db(&self) -> crate::db_types::votebase_catalog::ColumnUseKind {
		match self {
			ColumnUseKind::Query => crate::db_types::votebase_catalog::ColumnUseKind::Query,
			ColumnUseKind::Reference => crate::db_types::votebase_catalog::ColumnUseKind::Reference,
			ColumnUseKind::Both => crate::db_types::votebase_catalog::ColumnUseKind::Both,
		}
	}
}


#[derive(Debug, Eq, PartialEq, Hash)]
pub struct UseableObject {
	pub ruleset_path: String,
	pub object_name: String,
	pub db_object: UseableDbObject,
}
impl hashbrown::Equivalent<UseableObject> for (&str, &str) {
	fn equivalent(&self, key: &UseableObject) -> bool {
		key.ruleset_path.as_str() == self.0 && key.object_name.as_str() == self.1
	}
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum UseableDbObject {
	Table { columns: Vec<RoughColumn> },
	Function { is_action: bool, function_params: Vec<String>, return_type: String },
}
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RoughColumn {
	pub name: String,
	pub not_null: bool,
	pub pg_type_name: String,
	pub allowed_use: ColumnUseKind,
}


#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
	#[error("the migration for `{0}` doesn't match the provided schema")]
	InvalidMigration(String),
	#[error("a static child was indicated for a slot that doesn't already have one `{0}`")]
	UnspecifiedStaticChild(String),
	#[error("the typescript code has errors\n\n{0}")]
	InvalidTs(String),
	#[error("the uses of the rulesets have been violated\n\n{0:?}")]
	InvalidUses(Vec<String>),
	#[error("there are disallowed foreign keys\n\n{0:?}")]
	InvalidReferences(Vec<String>),

	#[error(transparent)]
	RuntimeRejected(#[from] RuntimeError),
	#[error(transparent)]
	Container(#[from] temp_container_utils::ContainerError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Db(#[from] postgres::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum CandidateApplyError {
	#[error("the candidate {0} couldn't be found")]
	CandidateNotFound(uuid::Uuid),
	#[error("the candidate ruleset {0} couldn't be read correctly from the database\n\n{1}")]
	CorruptedCandidate(uuid::Uuid, serde_json::Error),
	#[error("the candidate {0} was found to be invalid for the current state\n\n{1}")]
	Invalid(uuid::Uuid, ValidationError),
	#[error(transparent)]
	Db(#[from] postgres::Error),
}


struct ValidateCtx {
	db_container_name: String,
	reset_server_client: PgClient,
	reset_config: PgConfig,
	migrate_server_client: PgClient,
	migrate_config: PgConfig,
}

#[derive(Debug)]
struct StoredRuleset {
	ts_code: String,
	db_schema: String,
	// TODO when applying a ruleset, if there were uses that existed before that don't exist now, the grants for those old objects need to be revoked
	// if possible, just revoke *all* the old permissions and then grant the new ones afresh
	db_uses_functions: Vec<ConcreteFunctionUse>,
	db_uses_tables: Vec<ConcreteTableUse>,
	static_children: HashMap<String, StoredRuleset>,
	// static_recurring_events: HashMap<String, StaticRecurringEvent>,
}

async fn validate_bundled_ruleset_top(
	parent_full_path: Option<&str>, ruleset_name: &str,
	full_path: &str,
	prev_ruleset: Option<&StoredRuleset>,
	next_bundled_ruleset: &BundledRuleset,
	server_db_archive_path: &std::path::Path,
) -> Result<(), ValidationError> {
	temp_container_utils::with_temp_postgres_client(async |db_container_name, config, server_client| -> Result<(), ValidationError> {
		let reset_db_name = "tempdb|reset";
		let migrate_db_name = "tempdb|migrate";
		server_client.batch_execute(&format!(r#"create database "{reset_db_name}""#)).await?;
		server_client.batch_execute(&format!(r#"create database "{migrate_db_name}""#)).await?;
		drop(server_client);

		let reset_config = { let mut config = config.clone(); config.dbname(reset_db_name); config };
		let reset_server_client = crate::pg_con(&reset_config).await?;
		let migrate_config = { let mut config = config.clone(); config.dbname(migrate_db_name); config };
		let migrate_server_client = crate::pg_con(&migrate_config).await?;

		// UNWRAP SAFETY: it must be true that with_temp_postgres_client gives a client with a set user
		let db_user = config.get_user().unwrap();
		podman_pg_restore(&db_container_name, &reset_db_name, &db_user, server_db_archive_path).await?;
		podman_pg_restore(&db_container_name, &migrate_db_name, &db_user, server_db_archive_path).await?;

		let ctx = ValidateCtx {
			db_container_name, reset_server_client, reset_config, migrate_server_client, migrate_config,
		};

		validate_bundled_ruleset(full_path, parent_full_path, ruleset_name, &ctx, prev_ruleset, next_bundled_ruleset).await?;

		// TODO it would be nice to figure out how to try_join all these below queries

		// both possibly_effected_uses and existent_objects_by_ruleset_path_and_name come from the now updated state of the database
		let possibly_effected_uses = queries::rulesets::get_possibly_effected_uses().bind(&ctx.migrate_server_client)
			.map(|u| {
				let function_uses: Vec<ConcreteFunctionUse> =
					u.db_uses_functions.map(|f| ConcreteFunctionUse {
						ruleset_path: f.ruleset_path.to_string(),
						object_name: f.object_name.to_string(),
						is_action: f.is_action,
						params: f.param_types.map(|p| p.to_string()).collect(),
						return_type: f.return_type.to_string(),
					})
					.collect();

				let table_uses: Vec<ConcreteTableUse> =
					u.db_uses_tables.map(|t| ConcreteTableUse {
						ruleset_path: t.ruleset_path.to_string(),
						object_name: t.object_name.to_string(),
						columns: t.columns.map(|c| UseColumn {
							name: c.name.to_string(),
							pg_type: c.typ.to_string(),
							can_null: c.can_null,
							use_kind: c.use_kind.into(),
						}).collect(),
					})
					.collect();

				(u.using_full_path.to_string(), function_uses, table_uses)
			})
			.all().await?;

		let usable_columns = queries::reflection::get_usable_columns().bind(&ctx.migrate_server_client)
			.map(|u| UseableObject {
				ruleset_path: u.ruleset_path.to_string(),
				object_name: u.table_name.to_string(),
				db_object: UseableDbObject::Table {
					columns: u.columns.map(|col| RoughColumn {
						name: col.name.to_string(),
						not_null: col.not_null,
						pg_type_name: col.typ.to_string(),
						allowed_use: ColumnUseKind::Both, // TODO this will be determined later by the exposes system. for now it's always permissive
					}).collect(),
				},
			})
			.iter().await?;

		let usable_functions = queries::reflection::get_usable_functions().bind(&ctx.migrate_server_client)
			.map(|u| UseableObject {
				ruleset_path: u.ruleset_path.to_string(),
				object_name: u.function_name.to_string(),
				db_object: UseableDbObject::Function {
					is_action: u.is_action,
					function_params: u.function_params.map(str::to_string).collect(),
					return_type: u.return_type.to_string(),
				},
			})
			.iter().await?;

		use futures::TryStreamExt;
		use tokio_stream::StreamExt as TokioStreamExt;
		let existent_objects_by_ruleset_path_and_name = usable_columns.merge(usable_functions).try_collect().await?;
		// existent_objects_by_ruleset_path_and_name.insert(UseableObject {
		// 	ruleset_path: "votebase_catalog".to_string(), object_name: "candidate_replacement_ruleset".to_string(),
		// 	db_object: UseableDbObject::Table { can_reference: true, columns: vec![
		// 		RoughColumn { name: "candidate_for".to_string(), not_null: true, pg_type_name: "Text".to_string() },
		// 		RoughColumn { name: "bundled_ruleset".to_string(), not_null: true, pg_type_name: "Json".to_string() },
		// 	] },
		// });
		validate_schema_uses(&possibly_effected_uses, &existent_objects_by_ruleset_path_and_name)
			.map_err(ValidationError::InvalidUses)?;

		let foreign_key_references = queries::reflection::get_foreign_keys().bind(&ctx.migrate_server_client).all().await?;
		validate_schema_references(&foreign_key_references, table_references_by_ruleset_and_table_and_column)
			.map_err(ValidationError::InvalidReferences)?;

		Ok(())
	}).await?;


	Ok(())
}

pub async fn podman_votebase_tsc(full_ruleset_dir: &std::path::Path) -> Result<(), ValidationError> {
	let workspace_arg = format!("{}:/workspace/ruleset", full_ruleset_dir.to_string_lossy());
	let output = temp_container_utils::workspace_podman_run("votebase-tsc", &[], workspace_arg, &["--noEmit"]).await?;

	if !output.status.success() {
		// let all_output = output.stdout.extend(output.stderr);
		let errors = String::from_utf8_lossy(&output.stdout);
		Err(ValidationError::InvalidTs(errors.to_string()))
	}
	else { Ok(()) }
}

async fn validate_bundled_ruleset(
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
	crate::runtime::Runtime::new(&next_bundled_ruleset.ts_code).await?;

	let function_uses: Vec<_> = next_bundled_ruleset.db_uses_functions.iter().map(|u| {
		let params_vec: Vec<&str> = u.params.iter().map(AsRef::as_ref).collect();
		(u.ruleset_path.as_str(), u.object_name.as_str(), u.is_action, u.return_type.as_str(), params_vec)
	}).collect();

	let table_uses: Vec<_> = next_bundled_ruleset.db_uses_tables.iter().map(|u| {
		let columns_vec: Vec<DbUsesColumnStructBorrowed> = u.columns.iter().map(|c| DbUsesColumnStructBorrowed {
			name: &c.name,
			typ: &c.pg_type,
			can_null: c.can_null,
			use_kind: c.use_kind.into_db(),
		}).collect();
		(u.ruleset_path.as_str(), u.object_name.as_str(), columns_vec)
	}).collect();
	// TODO we have to make sure that if a references use is being dropped, that after the db_migration no items in the new ruleset in fact are still doing that reference
	// with unnested_confkey as (
	//   select oid, unnest(confkey) as confkey
	//   from pg_constraint
	// ),
	// unnested_conkey as (
	//   select oid, unnest(conkey) as conkey
	//   from pg_constraint
	// )
	// select
	//   c.conname as constraint_name,
	//   tbl.relname as constraint_table,
	//   col.attname as constraint_column,
	//   referenced_tbl.relname as referenced_table,
	//   referenced_field.attname as referenced_column,
	//   pg_get_constraintdef(c.oid) as definition
	// from pg_constraint c
	// left join unnested_conkey con on c.oid = con.oid
	// left join pg_class tbl on tbl.oid = c.conrelid
	// left join pg_attribute col on (col.attrelid = tbl.oid and col.attnum = con.conkey)
	// left join pg_class referenced_tbl on c.confrelid = referenced_tbl.oid
	// left join unnested_confkey conf on c.oid = conf.oid
	// left join pg_attribute referenced_field on (referenced_field.attrelid = c.confrelid and referenced_field.attnum = conf.confkey)
	// where c.contype = 'f';

	let formatted_ruleset_schema =
		if prev_ruleset.is_some() {
			// for reset, delete and recreate the ruleset
			// this delete is a cascade
			queries::rulesets::delete_ruleset().bind(&ctx.reset_server_client, &full_path).await?;
			let (_, formatted_ruleset_schema) = create_ruleset_validation(
				&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.ts_code, &next_bundled_ruleset.db_schema,
				&function_uses, &table_uses,
			).await?;

			// for migrate, just apply the migration!
			// TODO we need to modify the actual details of the ruleset to conform with what we're doing, similarly to what we'll do when replacing one
			let migrator_pass = queries::rulesets::get_ruleset_migrator().bind(&ctx.migrate_server_client, &full_path).one().await?;
			let migrator_config = { let mut c = ctx.migrate_config.clone(); c.password(migrator_pass); c };
			let migrator_client = crate::pg_con(&migrator_config).await?;
			migrator_client.batch_execute(&next_bundled_ruleset.db_migration).await?;

			formatted_ruleset_schema
		}
		else {
			// for entirely new rulesets the db_schema and db_migration should be the same!
			let (_, formatted_ruleset_schema) = create_ruleset_validation(
				&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.ts_code, &next_bundled_ruleset.db_schema,
				&function_uses, &table_uses,
			).await?;
			let (_, _) = create_ruleset_validation(
				&ctx.migrate_config, &ctx.migrate_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.ts_code, &next_bundled_ruleset.db_migration,
				&function_uses, &table_uses,
			).await?;

			formatted_ruleset_schema
		};

	let diff = podman_compute_diff(&ctx.db_container_name, &formatted_ruleset_schema, &ctx.reset_config, &ctx.migrate_config).await?;
	if !diff.is_empty() {
		return Err(ValidationError::InvalidMigration(full_path.to_string()))
	}

	Box::pin(
		validate_bundled_ruleset_children(
			full_path, &ctx,
			prev_ruleset.map(|r| &r.static_children), &next_bundled_ruleset.static_children,
		)
	).await?;

	Ok(())
}

async fn validate_bundled_ruleset_children(
	parent_full_path: &str,
	ctx: &ValidateCtx,
	prev_ruleset_children: Option<&HashMap<String, StoredRuleset>>,
	next_ruleset_children: &HashMap<String, KeepOrReplace<BundledRuleset>>,
) -> Result<(), ValidationError> {
	match prev_ruleset_children {
		// there were no children in the previous ruleset, or the previous ruleset didn't exist
		None => {
			for (child_ruleset_name, child_ruleset) in next_ruleset_children {
				if let KeepOrReplace::Replace(child_ruleset) = child_ruleset {
					let full_path = crate::format_full_path(Some(parent_full_path), child_ruleset_name);
					validate_bundled_ruleset(&full_path, Some(parent_full_path), child_ruleset_name, ctx, None, child_ruleset).await?;
				}
				else {
					return Err(ValidationError::UnspecifiedStaticChild(child_ruleset_name.to_owned()))
				}
			}

			Ok(())
		},
		// there were some children in the previous ruleset
		Some(prev_ruleset_children) => {
			use crate::{outer_join, JoinOption};
			let joined_children = outer_join(prev_ruleset_children, next_ruleset_children);
			for (child_ruleset_name, join_option) in joined_children {
				let full_path = crate::format_full_path(Some(parent_full_path), child_ruleset_name);

				match join_option {
					JoinOption::Left(prev_ruleset) => {
						recursively_delete_ruleset(&full_path, prev_ruleset, &ctx.reset_server_client).await?;
						recursively_delete_ruleset(&full_path, prev_ruleset, &ctx.migrate_server_client).await?;
					},

					JoinOption::Right(KeepOrReplace::Replace(next_ruleset)) => {
						validate_bundled_ruleset(&full_path, Some(parent_full_path), child_ruleset_name, ctx, None, next_ruleset).await?;
					},
					JoinOption::Right(KeepOrReplace::Keep) => {
						return Err(ValidationError::UnspecifiedStaticChild(child_ruleset_name.to_owned()))
					},

					JoinOption::Both(prev_ruleset, KeepOrReplace::Replace(next_ruleset)) => {
						validate_bundled_ruleset(&full_path, Some(parent_full_path), child_ruleset_name, ctx, Some(prev_ruleset), next_ruleset).await?;
					},
					JoinOption::Both(_prev_ruleset, KeepOrReplace::Keep) => {
						// do nothing! this branch of the tree remains as is
					},
				}
			}

			Ok(())
		},
	}
}

// we need to refactor this to also accept a list of foreign keys
fn validate_schema_uses(
	possibly_effected_uses: &Vec<(String, Vec<ConcreteFunctionUse>, Vec<ConcreteTableUse>)>,
	// TODO the Hash impl of UseableObject needs to only hash the ruleset path and name
	existent_objects_by_ruleset_path_and_name: &hashbrown::HashSet<UseableObject>,
) -> Result<(), Vec<String>> {
	let mut errors = vec![];

	for (using_ruleset_path, function_uses, table_uses) in possibly_effected_uses {

		for ConcreteTableUse { ruleset_path, object_name, columns: use_columns } in table_uses {
			let obj = existent_objects_by_ruleset_path_and_name.get(&(ruleset_path.as_str(), object_name.as_str()));
			if let Some(UseableObject { db_object: UseableDbObject::Table { columns: usable_columns }, .. }) = obj {
				for UseColumn { name: use_col_name, pg_type: use_col_pg_type_name, can_null, use_kind } in use_columns {
					match usable_columns.iter().find(|c| &c.name == use_col_name) {
						Some(usable_col) => {
							if &usable_col.pg_type_name != use_col_pg_type_name {
								errors.push(format!(
									"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name}.{use_col_name} as type {use_col_pg_type_name}, but available type is {}",
									usable_col.pg_type_name
								));
							}
							if usable_col.not_null && *can_null {
								errors.push(format!(
									"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name}.{use_col_name} expecting not null, but it's nullable",
								));
							}
							match (use_kind, &usable_col.allowed_use) {
								(_, ColumnUseKind::Both) => { /* everything is allowed */ },
								(ColumnUseKind::Both | ColumnUseKind::Query, ColumnUseKind::Reference) => {
									errors.push(format!(
										"ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name}.{use_col_name} to query, which isn't allowed",
									));
								},
								(ColumnUseKind::Both | ColumnUseKind::Reference, ColumnUseKind::Query) => {
									errors.push(format!(
										"ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name}.{use_col_name} to reference, which isn't allowed",
									));
								},
								_ => {}
							}
						},
						None => {
							errors.push(format!(
								"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name}.{use_col_name}, but that column is not available"
							));
						}
					}
				}
			}
			else if let Some(UseableObject { db_object: UseableDbObject::Function { .. }, .. }) = obj {
				errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name} as a table, but it's a function"));
			}
			else {
				errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name}, but it doesn't exist"));
			}
		}

		for ConcreteFunctionUse { ruleset_path, object_name, is_action: use_is_action, params: use_params, return_type: use_return_type } in function_uses {
			let obj = existent_objects_by_ruleset_path_and_name.get(&(ruleset_path.as_str(), object_name.as_str()));
			if let Some(UseableObject { db_object: UseableDbObject::Function { is_action: usable_is_action, function_params: usable_params, return_type: usable_return_type }, .. }) = obj {
				if *use_is_action && !usable_is_action {
					errors.push(format!("ruleset {using_ruleset_path} uses {ruleset_path}.{object_name} as volatile, but that isn't allowed"));
				}
				if use_return_type != usable_return_type {
					errors.push(format!("ruleset {using_ruleset_path} uses {ruleset_path}.{object_name} with return type {use_return_type}, but it returns {usable_return_type}"));
				}

				if use_params.len() != usable_params.len() {
					errors.push(format!(
						"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name} with {} params, but it has {} params",
						use_params.len(),
						usable_params.len(),
					));
					continue
				}
				for (i, (use_param_type, usable_param)) in use_params.iter().zip(usable_params.iter()).enumerate() {
					if use_param_type != usable_param {
						errors.push(format!(
							"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name} param {} as type {}, but available type is {}",
							i, use_param_type, usable_param,
						));
					}
				}
			}
			else if let Some(UseableObject { db_object: UseableDbObject::Table { .. }, .. }) = obj {
				errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name} as a function, but it's a table"));
			}
			else {
				errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name}, but it doesn't exist"));
			}
		}
	}

	if errors.len() > 0 { Err(errors) }
	else { Ok(()) }
}

fn validate_schema_references(
	foreign_key_references: &Vec<GetForeignKeys>,
	table_references_by_ruleset_and_table_and_column: &HashMap<(&str, &str, &str), UseColumn>,
) -> Result<(), Vec<String>> {
	let mut errors = vec![];

	use crate::queries::reflection::GetForeignKeys;
	for GetForeignKeys { using_ruleset_path, used_ruleset_path, table_name, column_name } in foreign_key_references {
		let table_use = table_references_by_ruleset_and_table_and_column.get(&(&used_ruleset_path, &table_name, &column_name));

		if let Some(UseColumn { use_kind: ColumnUseKind::Reference | ColumnUseKind::Both, .. }) = table_use {
			/* we're good, the key is justified by a reference use */
		}
		else if let Some(UseColumn { use_kind: ColumnUseKind::Query, .. }) = table_use {
			errors.push(format!(
				"ruleset {using_ruleset_path} still has a foreign key pointing to {used_ruleset_path}.{table_name}.{column_name}, but its declared \"use\" only allows it to query that column, not reference it",
			));
		}
		else {
			errors.push(format!(
				"ruleset {using_ruleset_path} still has a foreign key pointing to {used_ruleset_path}.{table_name}.{column_name}, but it doesn't have a declared \"use\" to reference that column",
			));
		}
	}

	if errors.len() > 0 { Err(errors) }
	else { Ok(()) }
}

async fn add_and_check_ruleset_grant_commands_top(
	server_client: &PgClient,
	grant_commands: &mut Vec<String>,
	possibly_effected_uses: &Vec<(String, Vec<ConcreteFunctionUse>, Vec<ConcreteTableUse>)>,
) -> Result<(), ValidationError> {

	let function_grants = queries::reflection::get_function_grants();
	let column_grants = queries::reflection::get_column_grants();
	let (function_grants, column_grants) = futures::try_join!(
		function_grants.bind(server_client).all(),
		column_grants.bind(server_client).all(),
	)?;

	add_and_check_ruleset_grant_commands(grant_commands, possibly_effected_uses, function_grants, column_grants)?;

	Ok(())
}

use crate::queries::reflection::{GetColumnGrants, GetFunctionGrants};



// after *all* of the migrations have been made to apply a candidate, we need to:
// check that the uses and actual references are valid
// - check that the uses in the final state are all valid, both that the object they point to exists and is of the right kind, and in the future whether that object is allowed to be used in that way
// - check that every actual column reference is backed up by a references use, basically that it's allowed (really this should just be folded into the existing checking/validating of uses)
// make sure the final state of the grants is correct, in a "clear all and replace" way (this produces a list of grant_commands we can batch_execute)
// - go through all *actual* grants and revoke them all, just to avoid confusion and do things the easy way
// - go through all the *new* uses and make grants for them all





// so we'll have a "check uses against permissions" function
// - it needs to grab the literal privileges (only care about column select/references) as well as any foreign key objects (we care about those because the references privilege is about *creating* referencing objects, not about *having* them)
// - it needs the uses obviously
// - we need to group together the referencing privileges along with the
// so I imagine a query with columns:
// using_schema (this comes from the role name, we have to extract the actual ruleset name from it),
// used_schema, table_name, column_name, has_select_priv, has_reference_priv, has_referencing_key
// - so we loop over the column privileges, and check if there's a *uses* that justifies it. if not we revoke it. if there's a referencing object that isn't justified we fail the ruleset. if there *is* a use justifying it but no corresponding privileges, we grant them
// importantly, this has to happen *after* the ruleset being *used* has been fully modified to its new form
// the reason for this is because it's fine if our grants get scrambled by the mutation, as long as we can look at what we *should* have after the update in the *using* ruleset, because that's what the rectification process is for

// this rectify_uses happens after *all* schema changes have been made, and it doesn't care at all about the *previous* uses, only the new ones, comparing against the *actual* permissions that currently exist
// are there any situations where we

fn add_and_check_ruleset_grant_commands(
	grant_commands: &mut Vec<String>,
	actual_function_uses: &HashMap<(&str, &str), ConcreteFunctionUse>,
	actual_column_uses: &HashMap<(&str, &str, &str), ConcreteTableUse>,
	function_grants: Vec<GetFunctionGrants>,
	column_grants: Vec<GetColumnGrants>,
) -> Result<(), ValidationError> {
	// essentially this needs to iterate through the *grants*, the ones that actually exist, and for each try to determine whether it *should* exist
	// if it shouldn't, we just revoke it, assuming that it previously had a valid uses but the object was renamed or something (and we don't have the ability to tell if such renames etc happened, at least without writing a postgres static analysis engine)
	// the only thing we actually flag as a *problem* is if there's a reference still in existence that is no longer justified by a uses, since that means that ruleset itself didn't clean it up. that's something we need to fail the validation for

	// we could also just revoke *all* existing grants and then issue commands to grant all the new ones at the end?
	// revoking a grant is cheap and won't error as long as the objects it's referencing exist, which here we know

	// here's a problem, we need to not only check the *grants* to see if they're justified, we also need to check *all* the foreign keys held by a ruleset to ensure it's justified
	// this ensures we catch ones that never made any sense

	// the easiest way to do this is just to revoke everything for all the rulesets a ruleset actually points to
	// although we still have figure out which ones it actually points to in order to not waste a huge amount of time
	// revoke all privileges on all tables in schema schema_name from role_name;
	// revoke all privileges on all sequences in schema schema_name from role_name;
	// revoke all privileges on all functions in schema schema_name from role_name;
	// revoke all privileges on all routines in schema schema_name from role_name;
	// revoke all privileges on schema schema_name from role_name;


	for GetFunctionGrants {
		 using_ruleset_path, used_ruleset_path, function_name, function_params, has_execute_priv,
	} in function_grants {
		// find out if this function grant is justified by a uses
		if let Some(function_use) = actual_function_uses.get(&(&using_schema, &function_name)) {
			// we need to be sure the types of the granted object actually correspond to the types of the use

		}
	}

	for GetColumnGrants {
		using_ruleset_path, used_ruleset_path, table_name, column_name, has_reference_priv, has_select_priv,
	} in column_grants {
		// find out if this column grant is justified by a uses
		let column_use = actual_column_uses.get(&(&using_schema, &table_name, &column_name));
	}

	Ok(())
}

enum GrantKind { Granting, Revoking }
fn add_function_grant_commands(
	grant_commands: &mut Vec<String>, granting: GrantKind,
	formatted_used_schema: &str, object_name: &str, is_action: bool,
	formatted_using_role_action: &str, formatted_using_role_view: &str,
) {
	// TODO should we allow the migrator role to call functions in foreign rulesets? I don't think we should
	if let GrantKind::Granting = granting {
		grant_commands.push(format!(r#"
			grant all permissions on function "{formatted_used_schema}"."{object_name}" to "{formatted_using_role_action}";
		"#));

		if !is_action { grant_commands.push(format!(r#"
			grant all permissions on function "{formatted_used_schema}"."{object_name}" to "{formatted_using_role_view}";
		"#)); }
	}
	else {
		grant_commands.push(format!(r#"
			revoke all permissions on function "{formatted_used_schema}"."{object_name}" from "{formatted_using_role_action}";
		"#));

		if !is_action { grant_commands.push(format!(r#"
			revoke all permissions on function "{formatted_used_schema}"."{object_name}" from "{formatted_using_role_view}";
		"#)); }
	}
}


async fn create_ruleset<'u>(
	base_config: &PgConfig, client: &mut PgClient,
	parent_full_path: Option<&str>, name: &str,
	ruleset_code: &str, db_schema: &str, fns: &Vec<(String, FnType)>,
	function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
	table_uses: &'u Vec<(&'u str, &'u str, Vec<DbUsesColumnStructBorrowed<'u>>)>,
) -> Result<PgClient, postgres::Error> {
	log::debug!("inserting ruleset");

	use db_generated::IterSql;
	let fns = IterSql(|| fns.iter().map(|(name, fn_type)| RulesetFnRawBorrowed { name, fn_type: *fn_type }));
	let function_uses = IterSql(||
		function_uses.iter().map(|(ruleset_path, object_name, is_action, return_type, params)|
			DbUsesFunctionStructParams {
				ruleset_path, object_name, is_action: *is_action, param_types: params.as_slice(), return_type,
			}
		)
	);
	let table_uses = IterSql(||
		table_uses.iter().map(|(ruleset_path, object_name, columns)|
			DbUsesTableStructParams {
				ruleset_path, object_name, columns: columns.as_slice(),
			}
		)
	);

	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &ruleset_code, &db_schema, &fns, &function_uses, &table_uses)
		.one().await?;

	let full_path = ruleset_row.full_path;
	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

	log::debug!("creating ruleset schema");
	let transaction = client.transaction().await?;
	let create_sql = format!(include_str!("./create-ruleset.sql"),
		formatted_ruleset_schema=format_ruleset_schema(&full_path),
		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
		formatted_ruleset_role_action=formatted_ruleset_role_action,
		formatted_ruleset_role_view=formatted_ruleset_role_view,
		migrator_pass=ruleset_row.migrator_pass,
		action_pass=ruleset_row.action_pass,
		view_pass=ruleset_row.view_pass,
	);
	transaction.batch_execute(&create_sql).await?;
	transaction.commit().await?;

	let mut migrator_config = base_config.clone();
	migrator_config.user(formatted_ruleset_role_migrator);
	migrator_config.password(ruleset_row.migrator_pass);

	log::debug!("applying ruleset schema");
	let migrator_client = pg_con(&migrator_config).await?;
	migrator_client.batch_execute(db_schema).await?;

	Ok(migrator_client)
}

async fn create_ruleset_validation<'u>(
	base_config: &PgConfig, client: &PgClient,
	parent_full_path: Option<&str>, name: &str,
	ruleset_code: &str, db_schema: &str, /*fns: &Vec<(String, FnType)>,*/
	function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
	table_uses: &'u Vec<(&'u str, &'u str, Vec<DbUsesColumnStructBorrowed<'u>>)>,
) -> Result<(PgClient, String), postgres::Error> {

	use db_generated::IterSql;
	// let fns = IterSql(|| fns.iter().map(|(name, fn_type)| RulesetFnRawBorrowed { name, fn_type: *fn_type }));
	let function_uses = IterSql(||
		function_uses.iter().map(|(ruleset_path, object_name, is_action, return_type, params)|
			DbUsesFunctionStructParams {
				ruleset_path, object_name, is_action: *is_action, param_types: params.as_slice(), return_type,
			}
		)
	);
	let table_uses = IterSql(||
		table_uses.iter().map(|(ruleset_path, object_name, columns)|
			DbUsesTableStructParams {
				ruleset_path, object_name, columns: columns.as_slice(),
			}
		)
	);

	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &ruleset_code, &db_schema, &(&[] as &[RulesetFnRawBorrowed<'static>]), &function_uses, &table_uses)
		.one().await?;

	let full_path = ruleset_row.full_path;
	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

	let formatted_ruleset_schema = format_ruleset_schema(&full_path);
	let create_sql = format!(include_str!("./create-ruleset.sql"),
		formatted_ruleset_schema=formatted_ruleset_schema,
		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
		formatted_ruleset_role_action=formatted_ruleset_role_action,
		formatted_ruleset_role_view=formatted_ruleset_role_view,
		migrator_pass=ruleset_row.migrator_pass,
		action_pass=ruleset_row.action_pass,
		view_pass=ruleset_row.view_pass,
	);
	client.batch_execute(&create_sql).await?;

	let mut migrator_config = base_config.clone();
	migrator_config.user(formatted_ruleset_role_migrator);
	migrator_config.password(ruleset_row.migrator_pass);
	let migrator_client = pg_con(&migrator_config).await?;
	migrator_client.batch_execute(db_schema).await?;

	Ok((migrator_client, formatted_ruleset_schema))
}


pub async fn apply_candidate_top(
	server_role_client: &mut PgClient,
	server_db_archive_path: &std::path::Path,
	candidate_id: &uuid::Uuid,
	delete_other_candidates: bool,
) -> Result<(), CandidateApplyError> {
	let (candidate_for, bundled_ruleset) = queries::rulesets::get_ruleset_candidate().bind(server_role_client, candidate_id)
		.map(|r| {
			let bundled_ruleset: BundledRuleset = serde_json::from_str(r.bundled_ruleset.0.get())?;
			Ok((r.candidate_for.to_string(), bundled_ruleset))
		})
		.opt().await?
		.ok_or_else(|| CandidateApplyError::CandidateNotFound(*candidate_id))?
		.map_err(|e| CandidateApplyError::CorruptedCandidate(*candidate_id, e))?;

	let prev_ruleset = get_stored_ruleset(&candidate_for).await?;
	let (parent_full_path, ruleset_name) = crate::split_full_path(&candidate_for);

	validate_bundled_ruleset_top(
		parent_full_path.as_deref(), &ruleset_name, &candidate_for,
		Some(&prev_ruleset), &bundled_ruleset, server_db_archive_path,
	).await.map_err(|e| CandidateApplyError::Invalid(*candidate_id, e))?;

	let prev_has_candidate_references = prev_ruleset.db_uses_tables.iter().any(|t| {
		t.ruleset_path == "votebase_catalog" && t.object_name == "candidate_replacement_ruleset"
		&& t.columns.iter().any(|c| c.use_kind == ColumnUseKind::Reference)
	});

	// we always have *at least* a root ruleset, so whenever apply_candidate_top is being called it's to replace a ruleset
	let mut server_role_tx = server_role_client.transaction().await?;
	apply_candidate(&mut server_role_tx, Some(prev_ruleset), bundled_ruleset).await?;

	// we always delete other candidates if the old ruleset can't possibly be referencing any of them
	// (which means it can't or at least shouldn't be using them in any continuous decision processes)
	if delete_other_candidates || !prev_has_candidate_references {
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

async fn apply_candidate(
	base_config: &PgConfig,
	server_role_tx: &mut postgres::Transaction<'_>,
	parent_full_path: Option<&str>, ruleset_name: &str, full_path: &str,
	prev_ruleset: Option<StoredRuleset>,
	bundled_ruleset: BundledRuleset,
) -> Result<(), CandidateApplyError> {
	let BundledRuleset { ts_code, db_schema, db_migration, db_uses_functions, db_uses_tables, static_children } = bundled_ruleset;

	let formatted_using_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_using_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_using_role_view = format_ruleset_role(&full_path, RoleType::View);

	// either migrate and update the ruleset if there was one previously...
	if let Some(prev_ruleset) = prev_ruleset {
		let migrator_pass = queries::rulesets::get_ruleset_migrator().bind(client, &full_path).one().await?;

		let migrator_config = { let mut c = base_config.clone(); c.user(formatted_using_role_migrator); c.password(migrator_pass); c };
		let migrator_client = crate::pg_con(&migrator_config).await?;
		migrator_client.batch_execute(&db_migration).await?;
		// remove all grants related to the uses of functions and tables
		// or do a "rectification" process ugh

		// rectification would go like this:
		// go through the old uses, and for each that doesn't have a corresponding equivalent, revoke it directly
		// for those that do have a new equivalent, if everything's the same do nothing, if they're different just make the changes implied by the differences
		// this is once again an outer_join problem, where we join all the function uses and table uses and then go through the pairings and update accordingly
		// since functions and tables aren't at all equivalent, if someone dropped a table/function and created a new one with that same name we're still okay, since that will be picked up as revoking the grant on the dropped thing (if even necessary? has the permission already disappeared when the item was dropped?) and granting entirely new permissions on the new thing
		// and since all these permissions can be granted/revoked granularly by column and permission, we're fine just issuing them all individually

		// all of this assumes that any ruleset we *use* already exists in the form we are updating it to! this is fine if children only reference ancestors, but not fine if people start doing sibling or cousin etc references
		// in the future we can fix this by constructing a dag from all the uses

		// this future dag will be even more complex
		// the dag relationships aren't based on the *uses* themselves, but rather when a use points to an object that is somehow changing or newly coming into existence
		// that's a better way to understand it: a dag pointer is only necessary if within the *current* application of the ruleset the thing it's pointing to is either changing or being created, because that means the thing needs to be created before we can issue the final grant for it
		// - if a ruleset uses an object that is going away and still needs it, that's a violation we'll catch in validation
		// - if a ruleset uses an object that is going away but will update to no longer use it, that that's fine, the deletion of the object will remove the permission for us
		// - if a ruleset uses an object that is being updated in some way, then the changes to the object mean we won't have to revoke the old permission? it does probably mean we have to issue a grant for the new updated thing *after* the update itself has already happened
		// the thing I'm most confused about is in what ways changes to an object invalidates grants that have already been made on it
		// I have to experiment to find out what the state of permissions is after doing something like updating a column, it's name or type or whatever


		let prev_db_uses_functions = prev_ruleset.db_uses_functions.into_iter()
			.map(|f| ((f.ruleset_path, f.object_name), (f.is_action, f.params, f.return_type))).collect::<HashMap<_, _>>();
		let prev_db_uses_tables = prev_ruleset.db_uses_tables.into_iter()
			.map(|f| ((f.ruleset_path, f.object_name), f.columns)).collect::<HashMap<_, _>>();

		let db_uses_functions = db_uses_functions.into_iter()
			.map(|f| ((f.ruleset_path, f.object_name), (f.is_action, f.params, f.return_type))).collect::<HashMap<_, _>>();
		let db_uses_tables = db_uses_tables.into_iter()
			.map(|f| ((f.ruleset_path, f.object_name), f.columns)).collect::<HashMap<_, _>>();

		let mut grant_commands = vec![];
		let joined_db_uses_functions = crate::outer_join(&prev_db_uses_functions, &db_uses_functions);
		for ((used_ruleset_path, object_name), opt) in joined_db_uses_functions {
			let formatted_used_schema = format_ruleset_schema(&used_ruleset_path);
			match opt {
				crate::JoinOption::Left((is_action, _, _)) => {
					add_function_grant_commands(&mut grant_commands, GrantKind::Granting, &formatted_used_schema, &object_name, *is_action, &formatted_using_role_action, &formatted_using_role_view);
				},
				crate::JoinOption::Right((is_action, params, return_type)) => {
					add_function_grant_commands(&mut grant_commands, GrantKind::Revoking, &formatted_used_schema, &object_name, *is_action, &formatted_using_role_action, &formatted_using_role_view);
				},
				crate::JoinOption::Both((prev_is_action, _, _), (is_action, _, _)) => {
					add_function_grant_commands(&mut grant_commands, GrantKind::Revoking, &formatted_used_schema, &object_name, *prev_is_action, &formatted_using_role_action, &formatted_using_role_view);
					add_function_grant_commands(&mut grant_commands, GrantKind::Granting, &formatted_used_schema, &object_name, *is_action, &formatted_using_role_action, &formatted_using_role_view);
				},
			}
		}

		let joined_db_uses_tables = crate::outer_join(&prev_db_uses_tables, &db_uses_tables);
		for ((used_ruleset_path, object_name), opt) in joined_db_uses_tables {
			let formatted_used_schema = format_ruleset_schema(&used_ruleset_path);
			match opt {
				crate::JoinOption::Left(columns) => {
					// columns.iter().map(|c| c.)
					// TODO similar to functions, but per-column
				},
				crate::JoinOption::Right(columns) => {
					// TODO similar to functions, but per-column
				},
				crate::JoinOption::Both(prev_columns, columns) => {
					// TODO similar to functions, but per-column
				},
			}
		}
	}
	// ... or create it from scratch
	else {
		let db_uses_functions: Vec<_> = db_uses_functions.iter().map(|u| {
			let params_vec: Vec<&str> = u.params.iter().map(AsRef::as_ref).collect();
			(u.ruleset_path.as_str(), u.object_name.as_str(), u.is_action, u.return_type.as_str(), params_vec)
		}).collect();

		let db_uses_tables: Vec<_> = db_uses_tables.iter().map(|u| {
			let columns_vec: Vec<DbUsesColumnStructBorrowed> = u.columns.iter().map(|c| DbUsesColumnStructBorrowed {
				name: &c.name,
				typ: &c.pg_type,
				can_null: c.can_null,
				use_kind: c.use_kind.into_db(),
			}).collect();
			(u.ruleset_path.as_str(), u.object_name.as_str(), columns_vec)
		}).collect();

		create_ruleset(
			base_config, client, parent_full_path, ruleset_name, &ts_code, &db_schema, fns, &db_uses_functions, &db_uses_tables,
		).await?;
	}

	apply_candidate_children(parent_full_path, prev_ruleset_children, next_ruleset_children).await?;

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

async fn apply_candidate_children(
	parent_full_path: &str,
	prev_ruleset_children: Option<&HashMap<String, StoredRuleset>>,
	next_ruleset_children: &HashMap<String, KeepOrReplace<BundledRuleset>>,
) -> Result<(), CandidateApplyError> {
	unimplemented!()
}

async fn get_stored_ruleset(full_path: &str) -> Result<StoredRuleset, postgres::Error> {
	unimplemented!()

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


// TODO is this necessary given the on delete cascade on parent_full_path?
// honestly a big part of me wants to *get rid* of that delete cascade, and do these deletes in a depth-first way
async fn recursively_delete_ruleset(
	full_path: &str,
	ruleset: &StoredRuleset,
	server_client: &PgClient,
) -> Result<(), postgres::Error> {
	queries::rulesets::delete_ruleset().bind(server_client, &full_path).await?;

	for (child_name, child_ruleset) in &ruleset.static_children {
		let child_full_path = crate::format_full_path(Some(full_path), &child_name);

		Box::pin(
			recursively_delete_ruleset(&child_full_path, &child_ruleset, server_client)
		).await?;
	}


	Ok(())
}

pub async fn propose_candidate_ruleset(
	parent_full_path: Option<&str>, ruleset_name: &str,
	prev_ruleset: Option<&StoredRuleset>,
	candidate: &BundledRuleset,
	server_role_client: &PgClient,
	server_db_archive_path: &std::path::Path,
) -> Result<uuid::Uuid, ValidationError> {
	let full_path = &format_full_path(parent_full_path, ruleset_name);

	validate_bundled_ruleset_top(
		parent_full_path, ruleset_name, full_path,
		prev_ruleset, candidate, server_db_archive_path,
	).await?;

	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(server_role_client, &full_path, &postgres::types::Json(candidate))
		.one().await?;

	Ok(candidate_uuid)
}
