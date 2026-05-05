use std::collections::HashMap;

// use db_generated::queries::reflection::GetForeignKeys;

use crate::podman_fns::{podman_compute_diff, podman_pg_restore};
use crate::runtime::RuntimeError;
use crate::{
	FnType, PgClient, PgConfig, RoleType, format_full_path, format_ruleset_role, format_ruleset_schema, pg_con, postgres, queries /*format_ruleset_schema*/
};
use crate::db_types::votebase_catalog::{
	RulesetFnBorrowed,
	// DbUsesFunctionStructParams, DbUsesTableStructParams, DbUsesColumnStructBorrowed,
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
	pub fns: Vec<(String, FnType)>,

	/// The final intended database schema.
	/// Used to check that `db_migration` does what it's intended to.
	pub db_schema: String,
	/// The migration intended to actually be run to reach the state of `db_schema`.
	/// This will be checked to ensure it actually goes from the *current* state of the `Ruleset` database to the one declared in `db_schema`.
	pub db_migration: String,
	// /// The fully qualified names and nature of all the database functions this `Ruleset` uses as its `requires`.
	// pub db_uses_functions: Vec<ConcreteFunctionUse>,
	// /// The fully qualified names and nature of all the database tables this `Ruleset` uses as its `requires`.
	// pub db_uses_tables: Vec<ConcreteTableUse>,

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


// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub struct ConcreteFunctionUse {
// 	pub ruleset_path: String,
// 	pub object_name: String,
// 	pub is_action: bool,
// 	pub params: Vec<String>,
// 	pub return_type: String,
// }

// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub struct ConcreteTableUse {
// 	pub ruleset_path: String,
// 	pub object_name: String,
// 	pub columns: Vec<UseColumn>,
// }
// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub struct UseColumn {
// 	pub name: String,
// 	pub pg_type: String,
// 	pub can_null: bool,
// 	pub use_kind: ColumnUseKind,
// }
// #[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq, Hash)]
// pub enum ColumnUseKind { Query, Reference, Both }
// impl From<crate::db_types::votebase_catalog::ColumnUseKind> for ColumnUseKind {
// 	fn from(value: crate::db_types::votebase_catalog::ColumnUseKind) -> Self {
// 		match value {
// 			crate::db_types::votebase_catalog::ColumnUseKind::Query => ColumnUseKind::Query,
// 			crate::db_types::votebase_catalog::ColumnUseKind::Reference => ColumnUseKind::Reference,
// 			crate::db_types::votebase_catalog::ColumnUseKind::Both => ColumnUseKind::Both,
// 		}
// 	}
// }
// impl ColumnUseKind {
// 	fn into_db(&self) -> crate::db_types::votebase_catalog::ColumnUseKind {
// 		match self {
// 			ColumnUseKind::Query => crate::db_types::votebase_catalog::ColumnUseKind::Query,
// 			ColumnUseKind::Reference => crate::db_types::votebase_catalog::ColumnUseKind::Reference,
// 			ColumnUseKind::Both => crate::db_types::votebase_catalog::ColumnUseKind::Both,
// 		}
// 	}
// }


// #[derive(Debug, Eq, PartialEq)]
// pub struct UseableObject {
// 	pub ruleset_path: String,
// 	pub object_name: String,
// 	pub db_object: UseableDbObject,
// }
// impl std::hash::Hash for UseableObject {
// 	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
// 		self.ruleset_path.hash(state);
// 		self.object_name.hash(state);
// 		// not db_object
// 	}
// }
// impl hashbrown::Equivalent<UseableObject> for (&str, &str) {
// 	fn equivalent(&self, key: &UseableObject) -> bool {
// 		key.ruleset_path.as_str() == self.0 && key.object_name.as_str() == self.1
// 	}
// }

// #[derive(Debug, Eq, PartialEq, Hash)]
// pub enum UseableDbObject {
// 	Table { columns: Vec<RoughColumn> },
// 	Function { is_action: bool, function_params: Vec<String>, return_type: String },
// }
// #[derive(Debug, Eq, PartialEq, Hash)]
// pub struct RoughColumn {
// 	pub name: String,
// 	pub not_null: bool,
// 	pub pg_type_name: String,
// 	pub allowed_use: ColumnUseKind,
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
	Container(#[from] temp_container_utils::ContainerError),
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
	#[error("the candidate ruleset {0} couldn't be read correctly from the database\n\n{1}")]
	CorruptedCandidate(uuid::Uuid, serde_json::Error),
	#[error("the candidate {0} was found to be invalid for the current state\n\n{1}")]
	Invalid(uuid::Uuid, ValidationError),

	#[error(transparent)]
	Db(#[from] postgres::Error),
}


pub(crate) struct ValidateCtx {
	db_container_name: String,
	reset_server_client: PgClient,
	reset_config: PgConfig,
	migrate_server_client: PgClient,
	migrate_config: PgConfig,
}

#[derive(Debug)]
pub struct StoredRuleset {
	pub ts_code: String,
	pub db_schema: String,
	// pub db_uses_functions: Vec<ConcreteFunctionUse>,
	// pub db_uses_tables: Vec<ConcreteTableUse>,
	// pub static_children: HashMap<String, StoredRuleset>,
	// static_recurring_events: HashMap<String, StaticRecurringEvent>,
}

pub async fn validate_bundled_ruleset_top(
	parent_full_path: Option<&str>, ruleset_name: &str,
	full_path: &str,
	prev_ruleset: Option<&StoredRuleset>,
	next_bundled_ruleset: &BundledRuleset,
	// server_db_archive_path: &std::path::Path,
) -> Result<(), ValidationError> {
	// TODO it's possible (and probably *only* possible) to check migrations if we check *the entire database*
	// - for the "migrate" db: start by loading the db archive, then just apply the migration(s)
	// - for the "reset" db: load and run *all* the db schemas for all the rulesets. the tricky thing here is doing them in order
	// for rulesets that are mutually dependent (which you intentionally want to allow!) you'll have to make it so the commands can all be run in order. the only way I can think of right now that achieves that is to require any tables that participate in these circular relationships to declare their foreign keys outside of the definition of the table, so you can parse the db_schemas and *fully* separate the table/function definitions from the foreign key definitions. that way you can put all the function definitions first (at least the ones with unchecked bodies), then the tables, then the foreign keys and other stuff

	temp_container_utils::with_temp_postgres_client(async |db_container_name, config, server_client| -> Result<(), ValidationError> {
		// set up the two separate checking dbs
		let reset_db_name = "tempdb|reset";
		let migrate_db_name = "tempdb|migrate";
		server_client.batch_execute(&format!(r#"create database "{reset_db_name}""#)).await?;
		server_client.batch_execute(&format!(r#"create database "{migrate_db_name}""#)).await?;
		drop(server_client);

		let reset_config = { let mut config = config.clone(); config.dbname(reset_db_name); config };
		let reset_server_client = crate::pg_con(&reset_config).await?;
		let migrate_config = { let mut config = config.clone(); config.dbname(migrate_db_name); config };
		let migrate_server_client = crate::pg_con(&migrate_config).await?;

		// // UNWRAP SAFETY: it must be true that with_temp_postgres_client gives a client with a set user
		// let db_user = config.get_user().unwrap();
		// podman_pg_restore(&db_container_name, &reset_db_name, &db_user, server_db_archive_path).await?;
		// podman_pg_restore(&db_container_name, &migrate_db_name, &db_user, server_db_archive_path).await?;

		let ctx = ValidateCtx {
			db_container_name, reset_server_client, reset_config, migrate_server_client, migrate_config,
		};

		// do the recursive validation, which is basically just application! but to two things at the same time
		validate_bundled_ruleset(full_path, parent_full_path, ruleset_name, &ctx, prev_ruleset, next_bundled_ruleset).await?;

		// TODO it would be nice to figure out how to try_join all these below queries

		// // grab the data needed to validate the uses state after doing the application
		// // both possibly_effected_uses and existent_objects_by_ruleset_path_and_name come from the now updated state of the database
		// let possibly_effected_uses = queries::rulesets::get_possibly_effected_uses().bind(&ctx.migrate_server_client)
		// 	.map(|u| {
		// 		let function_uses: Vec<ConcreteFunctionUse> =
		// 			u.db_uses_functions.map(|f| ConcreteFunctionUse {
		// 				ruleset_path: f.ruleset_path.to_string(),
		// 				object_name: f.object_name.to_string(),
		// 				is_action: f.is_action,
		// 				params: f.param_types.map(|p| p.to_string()).collect(),
		// 				return_type: f.return_type.to_string(),
		// 			})
		// 			.collect();

		// 		let table_uses: Vec<ConcreteTableUse> =
		// 			u.db_uses_tables.map(|t| ConcreteTableUse {
		// 				ruleset_path: t.ruleset_path.to_string(),
		// 				object_name: t.object_name.to_string(),
		// 				columns: t.columns.map(|c| UseColumn {
		// 					name: c.name.to_string(),
		// 					pg_type: c.typ.to_string(),
		// 					can_null: c.can_null,
		// 					use_kind: c.use_kind.into(),
		// 				}).collect(),
		// 			})
		// 			.collect();

		// 		(u.using_full_path.to_string(), function_uses, table_uses)
		// 	})
		// 	.all().await?;

		// let usable_columns = queries::reflection::get_usable_columns().bind(&ctx.migrate_server_client)
		// 	.map(|u| UseableObject {
		// 		ruleset_path: u.ruleset_path.to_string(),
		// 		object_name: u.table_name.to_string(),
		// 		db_object: UseableDbObject::Table {
		// 			columns: u.columns.map(|col| RoughColumn {
		// 				name: col.name.to_string(),
		// 				not_null: col.not_null,
		// 				pg_type_name: col.typ.to_string(),
		// 				allowed_use: ColumnUseKind::Both, // TODO this will be determined later by the exposes system. for now it's always permissive
		// 			}).collect(),
		// 		},
		// 	})
		// 	.iter().await?;

		// let usable_functions = queries::reflection::get_usable_functions().bind(&ctx.migrate_server_client)
		// 	.map(|u| UseableObject {
		// 		ruleset_path: u.ruleset_path.to_string(),
		// 		object_name: u.function_name.to_string(),
		// 		db_object: UseableDbObject::Function {
		// 			is_action: u.is_action,
		// 			function_params: u.function_params.map(str::to_string).collect(),
		// 			return_type: u.return_type.to_string(),
		// 		},
		// 	})
		// 	.iter().await?;

		// use futures::TryStreamExt;
		// use tokio_stream::StreamExt as TokioStreamExt;
		// let existent_objects_by_ruleset_path_and_name = usable_columns.merge(usable_functions).try_collect().await?;
		// // existent_objects_by_ruleset_path_and_name.insert(UseableObject {
		// // 	ruleset_path: "votebase_catalog".to_string(), object_name: "candidate_replacement_ruleset".to_string(),
		// // 	db_object: UseableDbObject::Table { can_reference: true, columns: vec![
		// // 		RoughColumn { name: "candidate_for".to_string(), not_null: true, pg_type_name: "Text".to_string() },
		// // 		RoughColumn { name: "bundled_ruleset".to_string(), not_null: true, pg_type_name: "Json".to_string() },
		// // 	] },
		// // });
		// validate_schema_uses(&possibly_effected_uses, &existent_objects_by_ruleset_path_and_name)
		// 	.map_err(ValidationError::InvalidUses)?;

		// let foreign_key_references = queries::reflection::get_foreign_keys().bind(&ctx.migrate_server_client).all().await?;
		// validate_schema_references(&foreign_key_references, table_references_by_ruleset_and_table_and_column)
		// 	.map_err(ValidationError::InvalidReferences)?;

		Ok(())
	}).await??;


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
	println!("podman tsc");
	podman_votebase_tsc(temp_dir.as_ref()).await?;

	// ensure the runtime is okay with it
	println!("trying runtime");
	crate::runtime::Runtime::new(&next_bundled_ruleset.ts_code).await?;

	// let function_uses: Vec<_> = next_bundled_ruleset.db_uses_functions.iter().map(|u| {
	// 	let params_vec: Vec<&str> = u.params.iter().map(AsRef::as_ref).collect();
	// 	(u.ruleset_path.as_str(), u.object_name.as_str(), u.is_action, u.return_type.as_str(), params_vec)
	// }).collect();

	// let table_uses: Vec<_> = next_bundled_ruleset.db_uses_tables.iter().map(|u| {
	// 	let columns_vec: Vec<DbUsesColumnStructBorrowed> = u.columns.iter().map(|c| DbUsesColumnStructBorrowed {
	// 		name: &c.name,
	// 		typ: &c.pg_type,
	// 		can_null: c.can_null,
	// 		use_kind: c.use_kind.into_db(),
	// 	}).collect();
	// 	(u.ruleset_path.as_str(), u.object_name.as_str(), columns_vec)
	// }).collect();

	// let formatted_ruleset_schema =
	// 	if prev_ruleset.is_some() {
	// 		// for reset, delete and recreate the ruleset
	// 		queries::rulesets::delete_ruleset().bind(&ctx.reset_server_client, &full_path).await?;
	// 		let (_, formatted_ruleset_schema) = create_ruleset_validation(
	// 			&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
	// 			&next_bundled_ruleset.ts_code, &next_bundled_ruleset.db_schema,
	// 			// &function_uses, &table_uses,
	// 		).await?;

	// 		// for migrate, just apply the migration!
	// 		// TODO we need to modify the actual details of the ruleset to conform with what we're doing, similarly to what we'll do when replacing one
	// 		let migrator_pass = queries::rulesets::get_ruleset_migrator().bind(&ctx.migrate_server_client, &full_path).one().await?;
	// 		let migrator_config = { let mut c = ctx.migrate_config.clone(); c.password(migrator_pass); c };
	// 		let migrator_client = crate::pg_con(&migrator_config).await?;
	// 		migrator_client.batch_execute(&next_bundled_ruleset.db_migration).await?;

	// 		formatted_ruleset_schema
	// 	}
	// 	else {
	// 		// for entirely new rulesets the db_schema and db_migration should be the same!
	// 		let (_, formatted_ruleset_schema) = create_ruleset_validation(
	// 			&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
	// 			&next_bundled_ruleset.ts_code, &next_bundled_ruleset.db_schema,
	// 			// &function_uses, &table_uses,
	// 		).await?;
	// 		let (_, _) = create_ruleset_validation(
	// 			&ctx.migrate_config, &ctx.migrate_server_client, parent_full_path, ruleset_name,
	// 			&next_bundled_ruleset.ts_code, &next_bundled_ruleset.db_migration,
	// 			// &function_uses, &table_uses,
	// 		).await?;

	// 		formatted_ruleset_schema
	// 	};

	// let diff = podman_compute_diff(&ctx.db_container_name, &formatted_ruleset_schema, &ctx.reset_config, &ctx.migrate_config).await?;
	// if !diff.is_empty() {
	// 	return Err(ValidationError::InvalidMigration(full_path.to_string()))
	// }

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


pub(crate) async fn create_ruleset<'u>(
	db_name: &str, client: &mut PgClient,
	parent_full_path: Option<&str>, name: &str,
	bundled_ruleset: &BundledRuleset,
	// function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
	// table_uses: &'u Vec<(&'u str, &'u str, Vec<DbUsesColumnStructBorrowed<'u>>)>,
) -> Result<(), postgres::Error> {
	println!("inserting ruleset");

	let BundledRuleset { ts_code, db_schema, db_migration, fns } = bundled_ruleset;
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
	println!("inserted ruleset");

	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);
	let formatted_ruleset_schema = format_ruleset_schema(&full_path);

	println!("creating ruleset schema");
	let mut transaction = client.transaction().await?;
	let create_sql = format!(include_str!("./create-ruleset.sql"),
		db_name=db_name,
		formatted_ruleset_schema=formatted_ruleset_schema,
		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
		formatted_ruleset_role_action=formatted_ruleset_role_action,
		formatted_ruleset_role_view=formatted_ruleset_role_view,
	);
	transaction.batch_execute(&create_sql).await?;
	println!("created ruleset schema");

	// TODO same role concerns
	// TODO also the alter role stuff doesn't count because postgres that only counts when you connect as that role ugh
	println!("applying ruleset schema");
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
	let (candidate_for, bundled_ruleset) = queries::rulesets::get_ruleset_candidate().bind(server_role_client, candidate_id)
		.map(|r| {
			let bundled_ruleset: BundledRuleset = serde_json::from_str(r.bundled_ruleset.0.get())?;
			Ok((r.candidate_for.to_string(), bundled_ruleset))
		})
		.opt().await?
		.ok_or_else(|| CandidateApplyError::CandidateNotFound(*candidate_id))?
		.map_err(|e| CandidateApplyError::CorruptedCandidate(*candidate_id, e))?;

	let prev_ruleset = get_stored_ruleset(&server_role_client, &candidate_for).await?;
	let (parent_full_path, ruleset_name) = crate::split_full_path(&candidate_for);

	validate_bundled_ruleset_top(
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
) -> Result<(), CandidateApplyError> {
	let BundledRuleset { ts_code, db_schema, db_migration, fns, /*db_uses_functions, db_uses_tables, static_children*/ } = bundled_ruleset;
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

	validate_bundled_ruleset_top(
		parent_full_path, ruleset_name, full_path,
		Some(&prev_ruleset), candidate,
		// server_db_archive_path,
	).await?;

	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(server_role_client, &full_path, &postgres::types::Json(candidate))
		.one().await?;

	Ok(candidate_uuid)
}
