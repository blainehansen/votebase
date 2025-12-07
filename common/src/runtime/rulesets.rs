use std::{collections::HashMap, cell::RefCell, rc::Rc};
use deno_core::OpState;
use crate::db_types::votebase_catalog::GranularityEnum;
use crate::runtime::RunInfo;
use crate::{PgClient, PgConfig, RoleType, format_ruleset_role, format_ruleset_schema, postgres, queries};

use super::{Runtime, RuntimeError, demand_external_allowed, run_err};


// when we want to propose a new ruleset, we need to:
// - validate it, which is the hard part:
// 	- make sure the typescript is well typed (it already needs to be bundled?)
// 	- make sure the migration is correct
// 	- check the way the `uses` and static children would impact the actual thing. we need to check that no uses from any *other* rulesets would be severed (:sad:)
// - insert it, which is easy, and basically is entirely delegated to the database function that actually does so

// when we want to replace a ruleset



#[derive(Debug, serde::Deserialize)]
struct BundledRuleset {
	/// Typescript code containing all the Actions and Views of the `Ruleset`.
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
	db_uses: Vec<String>,
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




// // the bootstrap process ensures there's always a ruleset at the root

// // rulesets can be created or otherwise instantiated in two ways:
// // - they can be fully created/inserted out of nowhere, such as when the bootstrap cli creates one
// // - they can be fully created/inserted out of nowhere, but with an actual parent present, such as is done with either a static or dynamic child
// // - they can be replaced, as in a ruleset can go from one thing to another thing

// // the fully created/inserted cases are all fully the same, it's just whether a parent ruleset is present (and therefore whether the ruleset we're creating relies on the granted visibility/executability of tables/functions from the parent)
// // the replacement cases are all the same. the ruleset itself is migrated (which might be tricky if objects the children depend on are being changed! the migrations simply have to account for and handle this, there's not much we can do about it, since going bottom up doesn't improve the situation) and then recursively all *static* children are either migrated or kept or deleted according to the mappings given in the candidate object



// pub async fn create_ruleset(
// 	base_config: &PgConfig, client: &mut PgClient,
// 	parent_full_path: Option<&str>, name: &str,
// 	action_names: &Vec<String>, view_names: &Vec<String>,
// 	ruleset_code: &str, db_schema: &str,
// ) -> Result<PgClient, postgres::Error> {
// 	println!("inserting ruleset");
// 	let ruleset_row = queries::rulesets::insert_ruleset()
// 		.bind(client, &parent_full_path, &name, &action_names, &view_names, &ruleset_code, &db_schema).one().await?;

// 	let full_path = ruleset_row.full_path;
// 	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
// 	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
// 	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

// 	println!("creating ruleset schema");
// 	let transaction = client.transaction().await?;
// 	let create_sql = format!(include_str!("./create-ruleset.sql"),
// 		formatted_ruleset_schema=format_ruleset_schema(&full_path),
// 		formatted_ruleset_role_migrator=formatted_ruleset_role_migrator,
// 		formatted_ruleset_role_action=formatted_ruleset_role_action,
// 		formatted_ruleset_role_view=formatted_ruleset_role_view,
// 		migrator_pass=ruleset_row.migrator_pass,
// 		action_pass=ruleset_row.action_pass,
// 		view_pass=ruleset_row.view_pass,
// 	);
// 	transaction.batch_execute(&create_sql).await?;
// 	transaction.commit().await?;

// 	let mut migrator_config = base_config.clone();
// 	migrator_config.user(formatted_ruleset_role_migrator);
// 	migrator_config.password(ruleset_row.migrator_pass);

// 	println!("applying ruleset schema");
// 	let (migrator_client, migrator_connection) = migrator_config.connect(postgres::NoTls).await?;
// 	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
// 	migrator_client.batch_execute(db_schema).await?;

// 	Ok(migrator_client)
// }

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

// pub async fn replace_ruleset(
// 	server_role_client: &impl db_generated::client::GenericClient,
// 	migrator_role_config: &PgConfig,
// 	new_ruleset_id: &uuid::Uuid,
// ) -> Result<(), postgres::Error> {
// 	let db_migration = queries::rulesets::apply_candidate().bind(server_role_client, new_ruleset_id).one().await?;

// 	let (migrator_client, migrator_connection) = migrator_role_config.connect(postgres::NoTls).await?;
// 	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
// 	migrator_client.batch_execute(&db_migration).await?;
// 	Ok(())
// }

// pub async fn delete_ruleset() -> () {
// 	unimplemented!()
// }


// // TODO use temp container dbs for these, which means you don't even need all this connection info!
// async fn validate_candidate(
// 	current_full_path: &str,
// 	server_role_config: &PgConfig,
// 	server_role_client: &PgClient,
// 	candidate: &BundledRuleset,
// ) -> Result<(Vec<String>, Vec<String>), RuntimeError> {
// 	let inner = Runtime::new(&candidate.code).await?;

// 	let inner_state = inner.js_runtime.op_state();
// 	let inner_state = inner_state.as_ref().borrow();
// 	let fn_map = inner_state.borrow::<super::VotebaseFnMap>();
// 	let mut actions = vec![];
// 	let mut views = vec![];
// 	for (fn_name, fn_type) in fn_map {
// 		match fn_type {
// 			super::VotebaseFn::Action(_) => { actions.push(fn_name.to_owned()) },
// 			super::VotebaseFn::View(_) => { views.push(fn_name.to_owned()) },
// 		}
// 	}

// 	let pgschema = format_ruleset_schema(current_full_path);
// 	let declared_full_path = &format!("{current_full_path}|declared");
// 	let (declared_tempdb_config, declared_dbname) = new_tempdb(&pgschema, declared_full_path, server_role_config, server_role_client)
// 		.await?;
// 	let intended_full_path = &format!("{current_full_path}|actual");
// 	let (actual_tempdb_config, actual_dbname) = new_tempdb(&pgschema, intended_full_path, server_role_config, server_role_client)
// 		.await?;

// 	let result = (|| async {
// 		let (declared_client, declared_conn) = declared_tempdb_config.connect(postgres::NoTls).await?;
// 		tokio::spawn(async move { if let Err(e) = declared_conn.await { log::error!("DB connection error: {}", e); } });
// 		declared_client.batch_execute(&format!(r#"create schema "{pgschema}";"#)).await?;
// 		declared_client.batch_execute(&candidate.db_schema).await?;

// 		let current_schema = compute_diff(&pgschema, &actual_tempdb_config, server_role_config).await?;
// 		let (actual_client, actual_conn) = actual_tempdb_config.connect(postgres::NoTls).await?;
// 		tokio::spawn(async move { if let Err(e) = actual_conn.await { log::error!("DB connection error: {}", e); } });
// 		actual_client.batch_execute(&current_schema).await?;
// 		actual_client.batch_execute(&candidate.db_migration).await?;

// 		let diff = compute_diff(&pgschema, &declared_tempdb_config, &actual_tempdb_config).await?;
// 		if !diff.is_empty() {
// 			log::error!("{}", diff);
// 			Err(RuntimeError::OtherError(format!("candidate for {current_full_path} has misdeclared schema")))
// 		}
// 		else { Ok(()) }
// 	})().await;

// 	let (drop_declared, drop_actual) = tokio::join!(
// 		async { drop_tempdb(declared_dbname, server_role_client).await },
// 		async { drop_tempdb(actual_dbname, server_role_client).await },
// 	);
// 	drop_declared?;
// 	drop_actual?;
// 	result?;

// 	Ok((actions, views))
// }

// const TEMP_DB_COMMENT: &'static str = "'TEMP DB CREATED BY votebase'";

// async fn new_tempdb(
// 	pgschema: &str,
// 	intended_full_path: &str,
// 	base_config: &PgConfig,
// 	base_client: &impl postgres::GenericClient,
// ) -> Result<(PgConfig, String), postgres::Error> {
// 	let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
// 	let intended_full_path = format_ruleset_schema(intended_full_path);
// 	let dbname = format!("temp_db:{intended_full_path}|{now}");

// 	// TODO is it possible to do this create database in the same query?
// 	base_client.batch_execute(&format!(r#"create database "{dbname}";"#)).await?;
// 	base_client.batch_execute(&format!(r#"
// 		alter database "{dbname}" set search_path = '{pgschema}';
// 		comment on database "{dbname}" is {TEMP_DB_COMMENT};
// 	"#)).await?;

// 	let mut config = base_config.clone();
// 	config.dbname(&dbname);
// 	Ok((config, dbname))
// }

// async fn drop_tempdb(dbname: String, base_client: &impl postgres::GenericClient) -> Result<(), postgres::Error> {
// 	base_client.batch_execute(&format!(r#"drop database if exists "{dbname}";"#)).await?;
// 	Ok(())
// }

// pub async fn compute_diff(
// 	pgschema: impl AsRef<str>,
// 	from_config: &PgConfig,
// 	to_config: &PgConfig,
// ) -> Result<String, RuntimeError> {
// 	// #[cfg(debug_assertions)]
// 	// let mut command = {
// 	// 	let mut command = tokio::process::Command::new("uv");
// 	// 	command.args("tool run -p 3.11 --with psycopg2-binary --with setuptools migra".split_whitespace());
// 	// 	command
// 	// };
// 	// #[cfg(not(debug_assertions))]
// 	// let mut command = tokio::process::Command::new("migra");

// 	let from_url = crate::url_encoded_connection_string(from_config);
// 	let to_url = crate::url_encoded_connection_string(to_config);

// 	let output = temp_container_utils::run_podman_cmd(
// 		"votebase-dbdiff",
// 		&[],
// 		// TODO
// 		// &["--network", &format!("container:{}", db_container_name.as_ref())],
// 		&["--with-privileges", "--schema", pgschema.as_ref(), &from_url, &to_url],
// 	).await?;

// 	// if !output.stderr.is_empty() {
// 	if !output.status.success() {
// 		let e = format!("dbdiff failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
// 		return Err(RuntimeError::OtherError(e));
// 	}
// 	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
// }


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
