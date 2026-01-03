use std::{collections::HashMap};

use crate::podman_fns::{podman_compute_diff, podman_pg_restore};
use crate::runtime::RuntimeError;
use crate::{
	postgres, queries, PgClient, PgConfig, RoleType,
	format_full_path, format_ruleset_role, format_ruleset_schema, /*format_ruleset_schema*/
};
use crate::db_types::votebase_catalog::{GranularityEnum, DbUsesFunctionStructParams, DbUsesTableStructParams, DbUsesColumnStructBorrowed};


// https://github.com/Aleph-Alpha/ts-rs
#[derive(Debug, serde::Deserialize)]
pub struct BundledRuleset {
	/// Typescript code containing all the Actions and Views of the `Ruleset`.
	/// This must be fully bundled, meaning it's the entire codebase of the whole `Ruleset`, including the queries code etc.
	/// It doesn't need to include the votebase runtime.
	pub code: String,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// pub fns: { [fn_name: string]: VotebaseFn<JsonValue> },
	// is there a world where the fns are all declared separately, and then some "shared code" chunk also? how to do this? create temp files for each to do all the checking?

	/// The final intended database schema.
	/// Used to check that `db_migration` does what it's intended to.
	pub db_schema: String,
	/// The migration intended to actually be run to reach the state of `db_schema`.
	/// This will be checked to ensure it actually goes from the *current* state of the `Ruleset` database to the one declared in `db_schema`.
	pub db_migration: String,
	/// The fully qualified names and nature of all the database objects this `Ruleset` uses as its `requires`.
	pub db_uses: Vec<ConcreteUse>,
	// TODO split these apart so it's cleaner? at the parsing stage they can be in the same "uses" and then pulled into separate things, but here it's not necessary
	// pub db_uses_functions: Vec<ConcreteFunctionUse>,
	// pub db_uses_tables: Vec<ConcreteTableUse>,

	/// A mapping of the static children of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	/// If this `Ruleset` replaces the existing one, this will be the absolute state of the static children, with any existing ones changed to match their new description and extra ones recursively deleted.
	pub static_children: HashMap<String, KeepOrReplace<BundledRuleset>>,
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

#[derive(Debug, serde::Deserialize)]
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

#[derive(Debug, serde::Deserialize)]
pub struct ConcreteUse {
	pub ruleset_path: String,
	pub object_name: String,
	pub use_kind: UseKind,
}

#[derive(Debug, serde::Deserialize)]
pub enum UseKind {
	Table { can_query: bool, columns: Vec<UseColumn> },
	Function { is_action: bool, params: Vec<String>, return_type: String },
}
#[derive(Debug, serde::Deserialize)]
pub struct UseColumn {
	pub name: String,
	pub pg_type: String,
	pub can_null: bool,
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
	Table { can_query: bool, columns: Vec<RoughColumn> },
	Function { is_action: bool, params: Vec<RoughParam>, return_type: String },
}
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RoughColumn {
	pub name: String,
	pub not_null: bool,
	pub pg_type_name: String,
}
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RoughParam {
	pub name: String,
	pub pg_type_name: String,
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

	#[error(transparent)]
	RuntimeRejected(#[from] RuntimeError),
	#[error(transparent)]
	Container(#[from] temp_container_utils::ContainerError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
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
	code: String,
	db_schema: String,
	db_uses: Vec<String>,
	static_children: HashMap<String, StoredRuleset>,
	// static_recurring_events: HashMap<String, StaticRecurringEvent>,
}

async fn validate_bundled_ruleset_top(
	parent_full_path: Option<&str>, ruleset_name: &str,
	full_path: &str,
	prev_bundled_ruleset: Option<&StoredRuleset>,
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

		validate_bundled_ruleset(full_path, parent_full_path, ruleset_name, &ctx, prev_bundled_ruleset, next_bundled_ruleset).await?;

		// TODO it would be nice to figure out how to try_join all these below queries

		// both possibly_effected_uses and existent_objects_by_ruleset_path_and_name come from the now updated state of the database
		let possibly_effected_uses = queries::rulesets::get_possibly_effected_uses().bind(&ctx.migrate_server_client)
			.map(|u| {
				let uses =
					u.db_uses_functions.map(|f| ConcreteUse {
						ruleset_path: f.ruleset_path.to_string(),
						object_name: f.object_name.to_string(),
						use_kind: UseKind::Function {
							is_action: f.is_action,
							params: f.params.map(|p| p.to_string()).collect(),
							return_type: f.return_type.to_string(),
						},
					})
					.chain(u.db_uses_tables.map(|t| ConcreteUse {
						ruleset_path: t.ruleset_path.to_string(),
						object_name: t.object_name.to_string(),
						use_kind: UseKind::Table {
							can_query: t.can_query,
							columns: t.columns.map(|c| UseColumn {
								name: c.name.to_string(),
								pg_type: c.typ.to_string(),
								can_null: c.can_null,
							}).collect(),
						},
					}))
					.collect();

				// this to_string() is probably not necessary in some arrangement, it seems possible to get possibly_effected_uses to be borrowing from the raw result or whatever's underneath this thing, but for now it's fine
				(u.using_full_path.to_string(), uses)
			})
			.all().await?;

		let usable_columns = queries::rulesets::get_usable_columns().bind(&ctx.migrate_server_client)
			.map(|u| UseableObject {
				ruleset_path: u.schema_name[8..].to_string(),
				object_name: u.table_name.to_string(),
				db_object: UseableDbObject::Table {
					can_query: true, // TODO this will be determined later by the exposes system. for now it's always true
					columns: u.columns.map(|col| RoughColumn {
						name: col.name.to_string(),
						not_null: col.not_null,
						pg_type_name: col.typ.to_string(),
					}).collect(),
				},
			})
			.iter().await?;

		let usable_functions = queries::rulesets::get_usable_functions().bind(&ctx.migrate_server_client)
			.map(|u| UseableObject {
				ruleset_path: u.schema_name[8..].to_string(),
				object_name: u.function_name.to_string(),
				db_object: UseableDbObject::Function {
					is_action: u.is_action,
					params: u.params.map(|param| RoughParam { name: param.name.to_string(), pg_type_name: param.typ.to_string() }).collect(),
					return_type: u.return_type.to_string(),
				},
			})
			.iter().await?;

		use futures::TryStreamExt;
		use tokio_stream::StreamExt as TokioStreamExt;
		let existent_objects_by_ruleset_path_and_name = usable_columns.merge(usable_functions).try_collect().await?;
		validate_schema_uses(&possibly_effected_uses, &existent_objects_by_ruleset_path_and_name)
			.map_err(ValidationError::InvalidUses)?;

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
	prev_bundled_ruleset: Option<&StoredRuleset>,
	next_bundled_ruleset: &BundledRuleset,
) -> Result<(), ValidationError> {
	// place the typescript code in a temp directory and check it
	let temp_dir = tmpdir::TmpDir::new("validate_bundled_ruleset").await?;
	let ts_file = temp_dir.as_ref().join("ruleset.ts");
	tokio::fs::write(&ts_file, &next_bundled_ruleset.code).await?;
	podman_votebase_tsc(temp_dir.as_ref()).await?;

	// ensure the runtime is okay with it
	crate::runtime::Runtime::new(&next_bundled_ruleset.code).await?;

	use itertools::Itertools;
	let (function_uses, table_uses): (Vec<_>, Vec<_>) = next_bundled_ruleset.db_uses.iter().partition_map(|u| {
		match &u.use_kind {
			UseKind::Function { is_action, params, return_type } => {
				let params_vec: Vec<&str> = params.iter().map(AsRef::as_ref).collect();
				itertools::Either::Left((u.ruleset_path.as_str(), u.object_name.as_str(), *is_action, return_type.as_str(), params_vec))
			},
			UseKind::Table { can_query, columns } => {
				let columns_vec: Vec<DbUsesColumnStructBorrowed> = columns.iter().map(|c| DbUsesColumnStructBorrowed {
					name: &c.name,
					typ: &c.pg_type,
					can_null: c.can_null,
				}).collect();
				itertools::Either::Right((u.ruleset_path.as_str(), u.object_name.as_str(), *can_query, columns_vec))
			},
		}
	});

	let formatted_ruleset_schema =
		if prev_bundled_ruleset.is_some() {
			// for reset, delete and recreate the ruleset
			// this delete is a cascade
			queries::rulesets::delete_ruleset().bind(&ctx.reset_server_client, &full_path).await?;
			let (_, formatted_ruleset_schema) = create_ruleset_validation(
				&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.code, &next_bundled_ruleset.db_schema,
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
				&next_bundled_ruleset.code, &next_bundled_ruleset.db_schema,
				&function_uses, &table_uses,
			).await?;
			let (_, _) = create_ruleset_validation(
				&ctx.migrate_config, &ctx.migrate_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.code, &next_bundled_ruleset.db_migration,
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
			prev_bundled_ruleset.map(|r| &r.static_children), &next_bundled_ruleset.static_children,
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

fn validate_schema_uses(
	possibly_effected_uses: &Vec<(String, Vec<ConcreteUse>)>,
	existent_objects_by_ruleset_path_and_name: &hashbrown::HashSet<UseableObject>,
) -> Result<(), Vec<String>> {
	let mut errors = vec![];

	for (using_ruleset_path, concrete_uses) in possibly_effected_uses {
		for ConcreteUse { ruleset_path, object_name, use_kind } in concrete_uses {
			let obj = existent_objects_by_ruleset_path_and_name.get(&(ruleset_path.as_str(), object_name.as_str()));
			if let Some(UseableObject { db_object, .. }) = obj {
				match (use_kind, db_object) {
					(
						UseKind::Table { can_query: use_can_query, columns: use_columns },
						UseableDbObject::Table { can_query: usable_can_query, columns: usable_columns },
					) => {
						if *use_can_query && !usable_can_query {
							errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name} as queryable, but that isn't allowed"));
							continue
						}

						for UseColumn { name: use_col_name, pg_type: use_col_pg_type_name, can_null } in use_columns {
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
											"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name}.{use_col_name} expecting not null, but available type is nullable",
										));
									}
								},
								None => {
									errors.push(format!(
										"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name}.{use_col_name}, but that column is not available"
									));
								}
							}
						}
					},

					(
						UseKind::Function { is_action: use_is_action, params: use_params, return_type: use_return_type },
						UseableDbObject::Function { is_action: usable_is_action, params: usable_params, return_type: usable_return_type },
					) => {
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
							if use_param_type != &usable_param.pg_type_name {
								errors.push(format!(
									"ruleset {using_ruleset_path} uses {ruleset_path}.{object_name} param {} (position {}) as type {}, but available type is {}",
									usable_param.name, i, use_param_type, usable_param.pg_type_name,
								));
							}
						}
					},

					_ => {
						// TODO this error needs to be more accurate and helpful
						errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name}, but it doesn't exist"));
					}
				}
			}
			else {
				// TODO this error needs to be more accurate and helpful
				errors.push(format!("ruleset {using_ruleset_path} tries to use {ruleset_path}.{object_name}, but it doesn't exist"));
			}
		}
	}

	if errors.len() > 0 { Err(errors) }
	else { Ok(()) }
}



pub async fn create_ruleset<'u>(
	base_config: &PgConfig, client: &mut PgClient,
	parent_full_path: Option<&str>, name: &str,
	action_names: &Vec<String>, view_names: &Vec<String>,
	ruleset_code: &str, db_schema: &str,
	function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
	table_uses: &'u Vec<(&'u str, &'u str, bool, Vec<DbUsesColumnStructBorrowed<'u>>)>,
) -> Result<PgClient, postgres::Error> {
	log::debug!("inserting ruleset");

	use db_generated::IterSql;
	let function_uses = IterSql(|| {
		function_uses.iter().map(|(ruleset_path, object_name, is_action, return_type, params)| {
			DbUsesFunctionStructParams {
				ruleset_path, object_name, is_action: *is_action, params: params.as_slice(), return_type,
			}
		})
	});

	let table_uses = IterSql(|| {
		table_uses.iter().map(|(ruleset_path, object_name, can_query, columns)| {
			DbUsesTableStructParams {
				ruleset_path, object_name, can_query: *can_query, columns: columns.as_slice(),
			}
		})
	});

	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &action_names, &view_names, &ruleset_code, &db_schema, &function_uses, &table_uses)
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
	let (migrator_client, migrator_connection) = migrator_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(db_schema).await?;

	Ok(migrator_client)
}

pub async fn create_ruleset_validation<'u>(
	base_config: &PgConfig, client: &PgClient,
	parent_full_path: Option<&str>, name: &str,
	ruleset_code: &str, db_schema: &str,
	function_uses: &'u Vec<(&'u str, &'u str, bool, &'u str, Vec<&'u str>)>,
	table_uses: &'u Vec<(&'u str, &'u str, bool, Vec<DbUsesColumnStructBorrowed<'u>>)>,
) -> Result<(PgClient, String), postgres::Error> {

	use db_generated::IterSql;
	let function_uses = IterSql(|| {
		function_uses.iter().map(|(ruleset_path, object_name, is_action, return_type, params)| {
			DbUsesFunctionStructParams {
				ruleset_path, object_name, is_action: *is_action, params: params.as_slice(), return_type,
			}
		})
	});

	let table_uses = IterSql(|| {
		table_uses.iter().map(|(ruleset_path, object_name, can_query, columns)| {
			DbUsesTableStructParams {
				ruleset_path, object_name, can_query: *can_query, columns: columns.as_slice(),
			}
		})
	});

	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &(&[] as &[String]), &(&[] as &[String]), &ruleset_code, &db_schema, &function_uses, &table_uses)
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

	let (migrator_client, migrator_connection) = migrator_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(db_schema).await?;

	Ok((migrator_client, formatted_ruleset_schema))
}


// TODO is this necessary given the on delete cascade on parent_full_path?
pub async fn recursively_delete_ruleset(
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
	prev_bundled_ruleset: Option<&StoredRuleset>,
	candidate: &BundledRuleset,
	server_role_config: &PgConfig,
	server_role_client: &PgClient,
	server_db_archive_path: &std::path::Path,
) -> Result<uuid::Uuid, ValidationError> {
	let full_path = &format_full_path(parent_full_path, ruleset_name);

	validate_bundled_ruleset_top(
		parent_full_path, ruleset_name, full_path,
		prev_bundled_ruleset, candidate, server_db_archive_path,
	).await?;

	// let client = server_role_pool.get().await?;
	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(server_role_client, &full_path, &actions, &views, &candidate.code, &candidate.db_schema, &candidate.db_migration)
		.one().await?;

	Ok(candidate_uuid)
}

pub async fn replace_ruleset(
	server_role_client: &PgClient,
	migrator_role_config: &PgConfig,
	new_ruleset_id: &uuid::Uuid,
) -> Result<(), postgres::Error> {
	let db_migration = queries::rulesets::apply_candidate().bind(server_role_client, new_ruleset_id).one().await?;

	// TODO perform replacments for all the children as well

	let (migrator_client, migrator_connection) = migrator_role_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(&db_migration).await?;
	Ok(())
}
