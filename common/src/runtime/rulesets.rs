use std::{collections::HashMap, cell::RefCell, rc::Rc};
use deno_core::OpState;
use crate::db_types::votebase_catalog::{GranularityEnum, DbUsesFunctionStructParams, DbUsesTableStructParams, DbUsesColumnStructBorrowed};
use crate::runtime::RunInfo;
use crate::{PgClient, PgConfig, RoleType, format_ruleset_role, format_ruleset_schema, postgres, queries};

use super::{Runtime, RuntimeError, demand_external_allowed, run_err};

async fn pg_con(config: &PgConfig) -> Result<PgClient, postgres::Error> {
	let (client, conn) = config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = conn.await { log::error!("DB connection error: {}", e); } });
	Ok(client)
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

async fn validate_bundled_ruleset_top(
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
		let reset_server_client = pg_con(&reset_config).await?;
		let migrate_config = { let mut config = config.clone(); config.dbname(migrate_db_name); config };
		let migrate_server_client = pg_con(&migrate_config).await?;

		let db_user = config.get_user().unwrap();
		podman_pg_restore(&db_container_name, &reset_db_name, &db_user, server_db_archive_path).await?;
		podman_pg_restore(&db_container_name, &migrate_db_name, &db_user, server_db_archive_path).await?;

		let ctx = ValidateCtx {
			db_container_name, reset_server_client, reset_config, migrate_server_client, migrate_config,
		};

		validate_bundled_ruleset(&full_path, &ctx, prev_bundled_ruleset, next_bundled_ruleset).await?;

		// TODO it would be nice to figure out how to try_join all these below queries

		// both possibly_effected_uses and existent_objects_by_ruleset_path_and_name come from the now updated state of the database
		// TODO but that's a problem, because the way we just constructed the migrate database isn't real! we need to put all the right information in the database so it can be fetched out at the end
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
	Runtime::new(&next_bundled_ruleset.code).await?;

	let formatted_ruleset_schema =
		if prev_bundled_ruleset.is_some() {
			// for reset delete and recreate the ruleset
			queries::rulesets::delete_ruleset().bind(&ctx.reset_server_client, &full_path).await?;
			let (_, formatted_ruleset_schema) = create_ruleset_validation(
				&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.code, &next_bundled_ruleset.db_schema,
				&next_bundled_ruleset.db_uses,
			).await?;

			// for migrate just apply the migration!
			let migrator_pass = queries::rulesets::get_ruleset_migrator().bind(&ctx.migrate_server_client, &full_path).one().await?;
			let migrator_config = { let mut c = ctx.migrate_config.clone(); c.password(migrator_pass); c };
			let migrator_client = pg_con(&migrator_config).await?;
			migrator_client.batch_execute(&next_bundled_ruleset.db_migration).await?;

			formatted_ruleset_schema
		}
		else {
			// for entirely new rulesets the db_schema and db_migration should be the same!
			let (_, formatted_ruleset_schema) = create_ruleset_validation(
				&ctx.reset_config, &ctx.reset_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.code, &next_bundled_ruleset.db_schema,
				&next_bundled_ruleset.db_uses,
			).await?;
			let (_, _) = create_ruleset_validation(
				&ctx.migrate_config, &ctx.migrate_server_client, parent_full_path, ruleset_name,
				&next_bundled_ruleset.code, &next_bundled_ruleset.db_migration,
				&next_bundled_ruleset.db_uses,
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
		None => {
			for (child_ruleset_name, child_ruleset) in next_ruleset_children {
				if let KeepOrReplace::Replace(child_ruleset) = child_ruleset {
					let full_path = crate::format_full_path(Some(parent_full_path), child_ruleset_name);
					validate_bundled_ruleset(&full_path, ctx, None, child_ruleset).await?;
				}
				else {
					return Err(ValidationError::UnspecifiedStaticChild(child_ruleset_name.to_owned()))
				}
			}

			Ok(())
		},
		Some(prev_ruleset_children) => {
			let joined_children = outer_join(prev_ruleset_children, next_ruleset_children);
			for (child_ruleset_name, join_option) in joined_children {
				let full_path = crate::format_full_path(Some(parent_full_path), child_ruleset_name);

				match join_option {
					JoinOption::Left(prev_ruleset) => {
						recursively_delete_ruleset(&full_path, prev_ruleset, &ctx.reset_server_client).await?;
						recursively_delete_ruleset(&full_path, prev_ruleset, &ctx.migrate_server_client).await?;
					},

					JoinOption::Right(KeepOrReplace::Replace(next_ruleset)) => {
						validate_bundled_ruleset(&full_path, ctx, None, next_ruleset).await?;
					},
					JoinOption::Right(KeepOrReplace::Keep) => {
						return Err(ValidationError::UnspecifiedStaticChild(child_ruleset_name.to_owned()))
					},

					JoinOption::Both(prev_ruleset, KeepOrReplace::Replace(next_ruleset)) => {
						validate_bundled_ruleset(&full_path, ctx, Some(prev_ruleset), next_ruleset).await?;
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

// // this function should already have had the "create ruleset" functionality called ahead of time, so the thing we actually have to pass down is a from_config and to_config that are set up for the migrator roles, since those are the ones
// async fn validate_bundled_ruleset(
// 	full_path: &str,
// 	prev_bundled_ruleset: Option<&BundledRuleset>,
// 	next_bundled_ruleset: &BundledRuleset,
// 	// this server_db_archive should be in a server wide mutex, since we're checking against it and don't want anyone to change it while we're operating
// 	db_container_name: &str,
// 	server_client: &PgClient,
// 	reset_config: &PgConfig,
// 	migrate_config: &PgConfig,
// ) -> Result<(), ValidationError> {

// 	// ### bundled_ruleset.code
// 	// place the typescript code in a temp directory and check it
// 	let temp_dir = tmpdir::TmpDir::new("validate_bundled_ruleset").await?;
// 	let ts_file = temp_dir.as_ref().join("ruleset.ts");
// 	tokio::fs::write(&ts_file, &next_bundled_ruleset.code).await?;
// 	podman_votebase_tsc(temp_dir.as_ref()).await?;

// 	// TODO also actually *run* the ruleset code to gather its fns and make sure they don't overlap, so Runtime considerations

// 	let formatted_ruleset_schema = format_ruleset_schema(full_path);

// 	// - "reset" has the "idealized" versions of each ruleset schema, which can be achieved by dropping each ruleset entirely, including roles, creating the ruleset schema, and then using the migrator to execute the db_schema
// 	// drop the ruleset schema entirely from the reset db
// 	// this only can be done if this ruleset already existed before
// 	server_client.batch_execute(&format!(r#"
// 		'drop schema "ruleset:{full_path}" cascade';
// 		'drop role "role:{full_path}|migrator"';
// 		'drop role "role:{full_path}|action"';
// 		'drop role "role:{full_path}|view"';
// 	"#)).await?;

// 	// - "migrate" has the "real" versions of each ruleset schema with the db_migration applied
// 	// after testing, it's clear that if you're going to drop the schema, you have to drop everything related to it, including all the roles, which means you need to recreate them using the create-ruleset once again!
// 	// doing a "drop schema cascade" will cascade to all *references*, meaning any foreign key constraints from other schemas will be simply dropped! this isn't a real problem, since this is just a comparison database, and the diff won't show up for the schema being *recreated*. and if there are children that reference the parent that initially had their constraints removed, when we get to them to perform the same diff, we'll do the same operation and therefore end up in the final desired state. this seems fine, it will work and not allow drift or spurious errors, hopefully
// 	// recursively *apply* all the migrations. for now we're doing this naively assuming all the rulesets haven't been changed to use any objects from any of their descendants, otherwise we'd have to do a dag ordering to apply all these migrations based on the uses
// 	// if the ruleset didn't exist before, we have more work to create it to ensure the roles we need to create it exist
// 	// then check the correctness of the migration for just this ruleset

// 	// then for each ruleset, you have to compare the old static children against the new static children. the cleanest thing is probably the below "outer join", so as you iterate through "current, next" you have options for both sides, meaning you know if you're creating from nothing (only right), modifying it (both), or deleting it (only left)
// 	// when deleting a child, you also have to recursively delete all its children!
// 	// the question is how do you get the old static children? and in their full form! that has to be saved somewhere. for now I guess I just have to assume its existence as I write these standalone functions. later I'll figure out where it's stored and pulled from

// 	// after applying all schema changes, load the entire body of usable objects, and recursively check that all the rulesets still have their uses satisfied




// 	// ### bundled_ruleset.db_schema
// 	// ### bundled_ruleset.db_migration
// 	let db_user = config.get_user().unwrap().to_string();

// 	// TODO this needs a different name? or you need to create these top level things at the top?
// 	// or maybe we can't avoid something like a "recursive schema update" process that recursively goes through the whole tree and applies all the database updates implied by the new ruleset, and then a separate "validation" pass that once again recursively goes through it and checks everything against the database, including all the diffs for each individual ruleset/schema. that probably makes much more sense.
// 	let from_db_name = "tempdb|from";
// 	let to_db_name = "tempdb|to";
// 	client.batch_execute(&format!(r#"create database "{from_db_name}""#)).await?;
// 	client.batch_execute(&format!(r#"create database "{to_db_name}""#)).await?;

// 	let from_config = { let mut config = config.clone(); config.dbname(from_db_name); config };
// 	let to_config = { let mut config = config.clone(); config.dbname(to_db_name); config };
// 	let (from_client, from_connection) = from_config.connect(postgres::NoTls).await?;
// 	tokio::spawn(async move { if let Err(e) = from_connection.await { log::error!("DB connection error: {}", e); } });
// 	let (to_client, to_connection) = to_config.connect(postgres::NoTls).await?;
// 	tokio::spawn(async move { if let Err(e) = to_connection.await { log::error!("DB connection error: {}", e); } });


// 	podman_pg_restore(&db_container_name, &from_db_name, &db_user, server_db_archive_path).await?;
// 	// TODO create_ruleset?
// 	// TODO these have to use the migrator configs for this ruleset, that's part of the validation
// 	from_client.batch_execute(&bundled_ruleset.db_schema).await?;

// 	podman_pg_restore(&db_container_name, &to_db_name, &db_user, server_db_archive_path).await?;
// 	to_client.batch_execute(&bundled_ruleset.db_migration).await?;

// 	let diff = podman_compute_diff(&db_container_name, &formatted_ruleset_schema, &from_config, &to_config).await?;
// 	if !diff.is_empty() {
// 		// log::error!("{}", diff);
// 		return Err(ValidationError::InvalidMigration(full_path.to_string()))
// 	}

// 	// here we go through all of these and actually *execute* all the implied changes to children!
// 	for (static_child_name, static_child) in &bundled_ruleset.static_children {
// 		match static_child {
// 			// - we make sure all the "keep" actually exist
// 			KeepOrReplace::Keep => { /* TODO make sure this static_child_name exists in the *current* paradigm */ },
// 			// - we make sure all the "replace" actually exist, and that they recursively make sense (have to be smart, we can't just call validate_bundled_ruleset again! we need to chop up this functionality so that we have a root call that starts the podman postgres and the others just perform actions)
// 			KeepOrReplace::Replace(static_child) => {
// 				let child_full_path = format!("{full_path}|{static_child_name}");
// 				validate_bundled_ruleset(&child_full_path, &static_child, server_db_archive_path, db_container_name, config, client).await?;
// 			},
// 		}
// 		// - we make sure that after our modifications and deletions everything still makes sense. this last one is something that probably happens only once at the top level, and includes checking our own db_uses
// 	}
// 	// these are easy, we just have to make sure they point to real actions that are in *this* ruleset (no references here), and that they're well-formed, which I'm pretty sure will have already happened at parse time
// 	bundled_ruleset.static_recurring_events;

// 	// TODO check that all the db_uses still make sense, along with validating that everything *else* in the system hasn't had uses orphaned
// 	bundled_ruleset.db_uses;
// 	Ok(())
// }

#[derive(Debug, serde::Deserialize)]
struct ConcreteUse {
	ruleset_path: String,
	object_name: String,
	use_kind: UseKind,
}

#[derive(Debug, serde::Deserialize)]
enum UseKind {
	Table { can_query: bool, columns: Vec<UseColumn> },
	Function { is_action: bool, params: Vec<String>, return_type: String },
}
#[derive(Debug, serde::Deserialize)]
struct UseColumn {
	name: String,
	pg_type: String,
	can_null: bool,
}

#[derive(Eq, PartialEq, Hash)]
struct UseableObject {
	ruleset_path: String,
	object_name: String,
	db_object: UseableDbObject,
}
#[derive(Eq, PartialEq, Hash)]
enum UseableDbObject {
	Table { can_query: bool, columns: Vec<RoughColumn> },
	Function { is_action: bool, params: Vec<RoughParam>, return_type: String },
}
#[derive(Eq, PartialEq, Hash)]
struct RoughColumn {
	name: String,
	not_null: bool,
	pg_type_name: String,
}
#[derive(Eq, PartialEq, Hash)]
struct RoughParam {
	name: String,
	pg_type_name: String,
}

impl hashbrown::Equivalent<UseableObject> for (&str, &str) {
	fn equivalent(&self, key: &UseableObject) -> bool {
		key.ruleset_path.as_str() == self.0 && key.object_name.as_str() == self.1
	}
}



fn construct_abstract_use_standin(use_kind: &UseKind) -> String {
	let dummy_name = "TODO".to_string();

	match use_kind {
		UseKind::Table { columns, .. } => {
			let columns_str = columns.iter().map(|UseColumn { name, pg_type, can_null }| {
				let null_portion = if *can_null { " not null" } else { "" };
				format!("{name} {pg_type}{null_portion}")
			}).collect::<Vec<_>>().join(", ");

			format!("create table {dummy_name} ({columns_str});")
		},
		UseKind::Function { is_action, params, return_type } => {
			let volatility = if *is_action { "volatile" } else { "stable" };
			let params_str = params.into_iter().enumerate()
				.map(|(idx, pg_type)| format!("_{idx} {pg_type}"))
				.collect::<Vec<_>>().join(", ");
			format!("create function {dummy_name}({params_str}) returns {return_type} as $$ begin raise exception ''; end; $$ language plpgsql {volatility};")
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

pub async fn podman_compute_diff(
	db_container_name: &str,
	target_schema: &str,
	from_config: &PgConfig,
	to_config: &PgConfig,
) -> std::io::Result<String> {
	let from_url = crate::url_encoded_connection_string(from_config);
	let to_url = crate::url_encoded_connection_string(to_config);

	let output = temp_container_utils::podman_run(
		"votebase-dbdiff",
		&["--network", &format!("container:{}", db_container_name)],
		&["--with-privileges", "--schema", target_schema, &from_url, &to_url],
	).await?;

	// if !output.stderr.is_empty() {
	if !output.status.success() {
		let e = format!("dbdiff failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(std::io::Error::other(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

async fn podman_pg_restore(
	db_container_name: &str,
	db_name: &str,
	db_user_name: &str,
	server_db_archive_path: &std::path::Path,
	// exclude_schema: Option<&str>,
) -> std::io::Result<()> {
	// pg_dump outputs to stdout if no --file argument is given, and pg_restore reads from stdin if no --file is given
	// if you go back to volumes: podman exec db_container_name pg_dump -d db_name -U db_user -f /whatever_volume_name/server_db_archive_path

	let mut command = tokio::process::Command::new("podman");
	command.arg("exec").arg(db_container_name)
		.arg("pg_restore")
		// --dbname=dbname
		.arg("-d").arg(db_name)
		// --username=username
		.arg("-U").arg(db_user_name)
		.arg("--format=custom")
		.arg("--schema-only")
		// --file=file
		// .arg("-f").arg(server_db_archive_path)
		.arg("--exit-on-error");

	// if let Some(exclude_schema) = exclude_schema {
	// 	// --exclude-schema=pattern
	// 	command.arg("-N").arg(exclude_schema);
	// }

	let mut child = command
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?;

	let mut server_db_archive = tokio::fs::File::open(server_db_archive_path).await?;
	let mut child_stdin = child.stdin.as_mut().ok_or_else(|| std::io::Error::other("unable to capture pg_restore stdin"))?;
	tokio::io::copy(&mut server_db_archive, &mut child_stdin).await?;

	let status = child.wait().await?;
	if status.success() { Ok(()) }
	else { Err(std::io::Error::other("pg_restore process failed")) }
}

async fn podman_pg_dump(
	db_container_name: &str,
	db_name: &str,
	db_user_name: &str,
	server_db_archive_path: &std::path::Path,
) -> std::io::Result<()> {
	let mut command = tokio::process::Command::new("podman");
	command.arg("exec").arg(db_container_name)
		.arg("pg_dump")
		// --dbname=dbname
		.arg("-d").arg(db_name)
		// --username=username
		.arg("-U").arg(db_user_name)
		.arg("--format=custom")
		.arg("--schema-only")
		// --file=file
		// .arg("-f").arg(server_db_archive_path)
		.arg("--exit-on-error");

	let mut child = command
		.stderr(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()?;

	let mut server_db_archive = tokio::fs::File::open(server_db_archive_path).await?;
	let mut child_stdout = child.stdout.as_mut().ok_or_else(|| std::io::Error::other("unable to capture pg_dump stdout"))?;
	tokio::io::copy(&mut child_stdout, &mut server_db_archive).await?;

	let status = child.wait().await?;
	if status.success() { Ok(()) }
	else { Err(std::io::Error::other("pg_dump process failed")) }
}


#[derive(Debug)]
struct StoredRuleset {
	code: String,
	db_schema: String,
	db_uses: Vec<String>,
	static_children: HashMap<String, StoredRuleset>,
	static_recurring_events: HashMap<String, StaticRecurringEvent>,
}

// https://github.com/Aleph-Alpha/ts-rs
#[derive(Debug, serde::Deserialize)]
struct BundledRuleset {
	/// Typescript code containing all the Actions and Views of the `Ruleset`.
	/// This must be fully bundled, meaning it's the entire codebase of the whole `Ruleset`, including the queries code etc.
	/// It doesn't need to include the votebase runtime.
	code: String,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// fns: { [fn_name: string]: VotebaseFn<JsonValue> },
	// is there a world where the fns are all declared separately, and then some "shared code" chunk also? how to do this? create temp files for each to do all the checking?

	/// The final intended database schema.
	/// Used to check that `db_migration` does what it's intended to.
	db_schema: String,
	/// The migration intended to actually be run to reach the state of `db_schema`.
	/// This will be checked to ensure it actually goes from the *current* state of the `Ruleset` database to the one declared in `db_schema`.
	db_migration: String,
	/// The fully qualified names of all the database objects this `Ruleset` uses as its `requires`.
	db_uses: Vec<ConcreteUse>,
	/// A mapping of the static children of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	/// If this `Ruleset` replaces the existing one, this will be the absolute state of the static children, with any existing ones changed to match their new description and extra ones recursively deleted.
	static_children: HashMap<String, KeepOrReplace<BundledRuleset>>,
	/// A mapping of the static recurring events of this `Ruleset`, with some being simply `"keep"`, meaning to leave it as is.
	/// If this `Ruleset` replaces the existing one, this will be the absolute state of the static recurring events, with any existing ones changed to match their new description and extra ones deleted.
	static_recurring_events: HashMap<String, KeepOrReplace<StaticRecurringEvent>>,

	// TODO dynamic children and and events is scope I'm cutting for now
	// /**
	//  * A predicate that determines what dynamic children to keep.
	//  * All others will be recursively deleted.
	// */
	// dynamic_children_keep_rule: String,
	// /**
	//  * A predicate that determines what dynamic recurring events to keep.
	//  * All others will be deleted.
	// */
	// dynamic_recurring_event_keep_rule: String,
	// /**
	//  * A predicate that determines what scheduled events to keep.
	//  * All others will be deleted.
	// */
	// dynamic_standalone_event_keep_rule: String,
}
// I don't actually think it's all that possible to have "blank placeholder types", especially when the structure of postgres types is so rigid in regards to how you actually construct them. if you want to do this, it needs to be similar to the table system in the sense that the actual *structure* of the type is what you have to declare you expect (composite, enumerated, range, base?)
// https://www.postgresql.org/docs/current/sql-createtype.html

// the *abstract* system will be structured roughly like this:
// "~v1": "ReferenceTable(c1 t1, c2 t2?)" // https://docs.rs/sqlparser/latest/sqlparser/ast/struct.TableAlias.html
// "~v2": "QueryAndReferenceTable(c1 t1?, c2 t2)"
// "~v3": "CallViewFunction(t1, t2) -> tr"
// "~v4": "CallActionFunction(t1, t2) -> tr"

// and the concrete fulfillments
// "~v1": "ruleset_path.table_name(c1, c2)"
// "~v2": "ruleset_path.table_name(c1, c2)"
// "~v3": "ruleset_path.function_name"
// "~v4": "ruleset_path.function_name"
// https://www.postgresql.org/docs/current/catalog-pg-proc.html


#[derive(Debug, serde::Deserialize)]
pub struct StaticRecurringEvent {
	description: String, start: chrono::DateTime<chrono::Utc>, recurrence_granularity: GranularityEnum, recurrence_multiplier: u16,
	action_name: String, action_arg: serde_json::Value,
}

#[derive(Debug, serde::Deserialize)]
pub enum KeepOrReplace<T> {
	Keep,
	Replace(T),
}


pub async fn create_ruleset<'u>(
	base_config: &PgConfig, client: &mut PgClient,
	parent_full_path: Option<&str>, name: &str,
	action_names: &Vec<String>, view_names: &Vec<String>,
	ruleset_code: &str, db_schema: &str,
	db_uses_functions: &Vec<DbUsesFunctionStructParams<'u>>, db_uses_tables: &Vec<DbUsesTableStructParams<'u>>,
) -> Result<PgClient, postgres::Error> {
	println!("inserting ruleset");
	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &action_names, &view_names, &ruleset_code, &db_schema, &db_uses_functions, &db_uses_tables).one().await?;

	let full_path = ruleset_row.full_path;
	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

	println!("creating ruleset schema");
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

	println!("applying ruleset schema");
	let (migrator_client, migrator_connection) = migrator_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(db_schema).await?;

	Ok(migrator_client)
}

pub async fn create_ruleset_validation(
	base_config: &PgConfig, client: &PgClient,
	parent_full_path: Option<&str>, name: &str,
	ruleset_code: &str, db_schema: &str,
	db_uses: &Vec<ConcreteUse>,
) -> Result<(PgClient, String), postgres::Error> {
	// Pre-collect the param slices for functions
	let function_params: Vec<Vec<&str>> = db_uses.iter().filter_map(|u| {
		match &u.use_kind {
			UseKind::Function { params, .. } => Some(params.iter().map(AsRef::as_ref).collect()),
			_ => None,
		}
	}).collect();

	// Pre-collect the column slices for tables
	let table_columns: Vec<Vec<DbUsesColumnStructBorrowed>> = db_uses.iter().filter_map(|u| {
		match &u.use_kind {
			UseKind::Table { columns, .. } => Some(columns.iter().map(|c| DbUsesColumnStructBorrowed {
				name: &c.name,
				typ: &c.pg_type,
				can_null: c.can_null,
			}).collect()),
			_ => None,
		}
	}).collect();

	// Collect the actual structs
	let db_uses_functions: Vec<DbUsesFunctionStructParams> = db_uses.iter()
		.filter_map(|u| match &u.use_kind {
			UseKind::Function { is_action, return_type, .. } => Some((u, is_action, return_type)),
			_ => None,
		})
		.zip(function_params.iter())
		.map(|((u, is_action, return_type), params)| DbUsesFunctionStructParams {
			ruleset_path: &u.ruleset_path,
			object_name: &u.object_name,
			is_action: *is_action,
			params: params.as_slice(),
			return_type: return_type,
		})
		.collect();

	let db_uses_tables: Vec<DbUsesTableStructParams> = db_uses.iter()
		.filter_map(|u| match &u.use_kind {
			UseKind::Table { can_query, .. } => Some((u, can_query)),
			_ => None,
		})
		.zip(table_columns.iter())
		.map(|((u, can_query), columns)| DbUsesTableStructParams {
			ruleset_path: &u.ruleset_path,
			object_name: &u.object_name,
			can_query: *can_query,
			columns: columns.as_slice(),
		})
		.collect();

	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &(&[] as &[String]), &(&[] as &[String]), &ruleset_code, &db_schema, &db_uses_functions, &db_uses_tables).one().await?;

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

pub async fn propose_candidate_ruleset(
	current_full_path: &str,
	server_role_config: &PgConfig,
	server_role_client: &PgClient,
	candidate: &BundledRuleset,
) -> Result<uuid::Uuid, RuntimeError> {
	let (actions, views) = validate_candidate(current_full_path, server_role_config, server_role_client, candidate).await?;

	// let client = server_role_pool.get().await?;
	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(server_role_client, &current_full_path, &actions, &views, &candidate.code, &candidate.db_schema, &candidate.db_migration)
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

// op_propose_self_replacement: (candidate: BundledRuleset) => Promise<string>,
#[deno_core::op2(async, reentrant)]
#[string]
pub async fn op_propose_self_replacement(
	state: Rc<RefCell<OpState>>,
	#[serde] candidate: BundledRuleset,
) -> Result<String, deno_error::JsErrorBox> {
	let state = state.as_ref().borrow();
	demand_external_allowed(&state)?;
	let run_info = state.borrow::<RunInfo>();
	let server_role_config = state.borrow::<ServerRoleConfig>();
	let server_role_client = state.borrow::<PgClient>();

	let candidate_uuid = propose_candidate_ruleset(
		&run_info.current_ruleset_path, &server_role_config.0, server_role_client, &candidate,
	).await.map_err(run_err)?;

	Ok(candidate_uuid.to_string())
}

// // op_create_child_ruleset: (
// // 	name: string, initial: ConcreteRuleset,
// // 	// tables in the parent ruleset that the view role of the child ruleset are granted select and the migrator role is granted references to
// // 	allowed_view_tables: string[],
// // 	// functions in the parent ruleset that the action role of the child ruleset are granted execute
// // 	allowed_action_functions: string[],
// // ) => Promise<string>,
// #[deno_core::op2(async, reentrant)]
// #[string]
// async fn op_create_child_ruleset(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] name: String,
// 	#[serde] initial: rulesets::ConcreteRuleset,
// 	#[serde] allowed_view_tables: Vec<String>,
// 	#[serde] allowed_action_functions: Vec<String>,
// ) -> String {
// 	unimplemented!()
// }

// // op_delete_child_ruleset: (name: string) => Promise<void>,
// #[deno_core::op2(async, reentrant)]
// #[string]
// async fn op_delete_child_ruleset(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] name: String,
// ) -> String {
// 	unimplemented!()
// }

// // this is just the child version of op_propose_self_replacement
// op_propose_child_replacement: (name: string, candidate: BundledRuleset) => Promise<string>,
// // the table with candidate_id already has the name and full_path etc to know where it's headed
// op_replace_child: (candidate_id: string) => Promise<void>,


enum JoinOption<T1, T2> {
	Left(T1),
	Right(T2),
	Both(T1, T2),
}

fn outer_join<'a, V1, V2>(
	map1: &'a HashMap<String, V1>,
	map2: &'a HashMap<String, V2>
) -> HashMap<&'a String, JoinOption<&'a V1, &'a V2>> {
	let mut result = HashMap::new();
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
