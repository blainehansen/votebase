use std::collections::HashMap;
use crate::db_types::votebase_catalog::GranularityEnum;
use crate::{queries, PgConfig, PgPool, PgClient, RoleType, format_ruleset_schema, format_ruleset_role, postgres};

use super::{Runtime, RuntimeError};

// the bootstrap process ensures there's always a ruleset at the root

// rulesets can be created or otherwise instantiated in two ways:
// - they can be fully created/inserted out of nowhere, such as when the bootstrap cli creates one
// - they can be fully created/inserted out of nowhere, but with an actual parent present, such as is done with either a static or dynamic child
// - they can be replaced, as in a ruleset can go from one thing to another thing

// the fully created/inserted cases are all fully the same, it's just whether a parent ruleset is present (and therefore whether the ruleset we're creating relies on the granted visibility/executability of tables/functions from the parent)
// the replacement cases are all the same. the ruleset itself is migrated (which might be tricky if objects the children depend on are being changed! the migrations simply have to account for and handle this, there's not much we can do about it, since going bottom up doesn't improve the situation) and then recursively all *static* children are either migrated or kept or deleted according to the mappings given in the candidate object


#[derive(Debug, serde::Deserialize)]
pub struct ConcreteRuleset {
	pub code: String,
	pub db_schema: String,
	pub db_migration: String,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// fns: { [fn_name: string]: VotebaseFn<JsonValue> },

	pub static_children: HashMap<String, ConcreteRuleset>,

	pub static_recurring_actions: Vec<StaticRecurringAction>,
}

#[derive(Debug,serde::Deserialize)]
pub enum KeepOrReplace<T> {
	Keep,
	Replace(T),
}

#[derive(Debug, serde::Deserialize)]
pub struct CandidateRuleset {
	pub code: String,
	pub db_schema: String,
	pub db_migration: String,

	// this is truly harvested from the code, but honestly it might be a good idea to also require a declaration we can check against
	// fns: { [fn_name: string]: VotebaseFn<JsonValue> },

	pub static_children: HashMap<String, KeepOrReplace<CandidateRuleset>>,

	pub static_recurring_actions: Vec<StaticRecurringAction>,
}

#[derive(Debug, serde::Deserialize)]
pub struct StaticRecurringAction {
	pub description: String, pub start: chrono::DateTime<chrono::Utc>, pub recurrence_granularity: GranularityEnum, pub recurrence_multiplier: i16,
	pub action_name: String, pub action_arg: serde_json::Value,
}


pub async fn create_ruleset(
	base_config: &PgConfig, client: &mut PgClient,
	parent_full_path: Option<&str>, name: &str,
	action_names: &Vec<String>, view_names: &Vec<String>,
	ruleset_code: &str, db_schema: &str,
) -> Result<(), postgres::Error> {
	let ruleset_row = queries::rulesets::insert_ruleset()
		.bind(client, &parent_full_path, &name, &action_names, &view_names, &ruleset_code, &db_schema).one().await?;

	let full_path = ruleset_row.full_path;
	let formatted_ruleset_role_migrator = format_ruleset_role(&full_path, RoleType::Migrator);
	let formatted_ruleset_role_action = format_ruleset_role(&full_path, RoleType::Action);
	let formatted_ruleset_role_view = format_ruleset_role(&full_path, RoleType::View);

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

	let (migrator_client, migrator_connection) = migrator_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(db_schema).await?;

	Ok(())
}

pub async fn propose_candidate_ruleset(
	current_full_path: &String,
	server_pg_config: &PgConfig,
	server_role_pool: &PgPool,
	server_pg_client: &PgClient,
	candidate: &CandidateRuleset,

) -> Result<uuid::Uuid, RuntimeError> {
	let (actions, views) = validate_candidate(current_full_path, &server_pg_config, &server_pg_client, &candidate).await?;

	let client = server_role_pool.get().await?;
	let candidate_uuid = queries::rulesets::insert_candidate_replacement()
		.bind(&client, &current_full_path, &actions, &views, &candidate.code, &candidate.db_schema, &candidate.db_migration)
		.one().await?;

	Ok(candidate_uuid)
}

pub async fn replace_ruleset(
	server_client: &PgClient,
	migrator_role_config: PgConfig,
	new_ruleset_id: uuid::Uuid,
) -> Result<(), postgres::Error> {
	let db_migration = queries::rulesets::apply_candidate().bind(server_client, &new_ruleset_id).one().await?;

	let (migrator_client, migrator_connection) = migrator_role_config.connect(postgres::NoTls).await?;
	tokio::spawn(async move { if let Err(e) = migrator_connection.await { log::error!("DB connection error: {}", e); } });
	migrator_client.batch_execute(&db_migration).await?;
	Ok(())
}

pub async fn delete_ruleset() -> () {
	unimplemented!()
}


// TODO all of this makes me nervous for performance. the repeated connecting over and over
// it would be nice to have a separate database server for this kind of analysis?
async fn validate_candidate(
	current_full_path: &str,
	server_pg_config: &PgConfig,
	server_pg_client: &PgClient,
	candidate: &CandidateRuleset,
) -> Result<(Vec<String>, Vec<String>), RuntimeError> {
	let inner = Runtime::new(&candidate.code).await?;

	let inner_state = inner.js_runtime.op_state();
	let inner_state = inner_state.as_ref().borrow();
	let fn_map = inner_state.borrow::<crate::runtime::VotebaseFnMap>();
	let mut actions = vec![];
	let mut views = vec![];
	for (fn_name, fn_type) in fn_map {
		match fn_type {
			super::VotebaseFn::Action(_) => { actions.push(fn_name.to_owned()) },
			super::VotebaseFn::View(_) => { views.push(fn_name.to_owned()) },
		}
	}

	let pgschema = format_ruleset_schema(current_full_path);
	let declared_full_path = &format!("{current_full_path}|declared");
	let (declared_tempdb_config, declared_dbname) = new_tempdb(&pgschema, declared_full_path, server_pg_config, server_pg_client)
		.await?;
	let intended_full_path = &format!("{current_full_path}|actual");
	let (actual_tempdb_config, actual_dbname) = new_tempdb(&pgschema, intended_full_path, server_pg_config, server_pg_client)
		.await?;

	let result = (|| async {
		let (declared_client, declared_conn) = declared_tempdb_config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = declared_conn.await { log::error!("DB connection error: {}", e); } });
		declared_client.batch_execute(&format!(r#"create schema "{pgschema}";"#)).await?;
		declared_client.batch_execute(&candidate.db_schema).await?;

		let current_schema = compute_diff(&pgschema, &actual_tempdb_config, server_pg_config).await?;
		let (actual_client, actual_conn) = actual_tempdb_config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = actual_conn.await { log::error!("DB connection error: {}", e); } });
		actual_client.batch_execute(&current_schema).await?;
		actual_client.batch_execute(&candidate.db_migration).await?;

		let diff = compute_diff(&pgschema, &declared_tempdb_config, &actual_tempdb_config).await?;
		if !diff.is_empty() {
			log::error!("{}", diff);
			Err(RuntimeError::OtherError(format!("candidate for {current_full_path} has misdeclared schema")))
		}
		else { Ok(()) }
	})().await;

	let (drop_declared, drop_actual) = tokio::join!(
		async { drop_tempdb(declared_dbname, server_pg_client).await },
		async { drop_tempdb(actual_dbname, server_pg_client).await },
	);
	drop_declared?;
	drop_actual?;
	result?;

	Ok((actions, views))
}

const TEMP_DB_COMMENT: &'static str = "'TEMP DB CREATED BY votebase'";

async fn new_tempdb(
	pgschema: &str,
	intended_full_path: &str,
	base_config: &PgConfig,
	base_client: &PgClient,
) -> Result<(PgConfig, String), postgres::Error> {
	let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
	let intended_full_path = format_ruleset_schema(intended_full_path);
	let dbname = format!("temp_db:{intended_full_path}|{now}");

	// TODO is it possible to do this create database in the same query?
	base_client.batch_execute(&format!(r#"create database "{dbname}";"#)).await?;
	base_client.batch_execute(&format!(r#"
		alter database "{dbname}" set search_path = '{pgschema}';
		comment on database "{dbname}" is {TEMP_DB_COMMENT};
	"#)).await?;

	let mut config = base_config.clone();
	config.dbname(&dbname);
	Ok((config, dbname))
}

async fn drop_tempdb(dbname: String, base_client: &PgClient) -> Result<(), postgres::Error> {
	base_client.batch_execute(&format!(r#"drop database if exists "{dbname}";"#)).await?;
	Ok(())
}

async fn compute_diff(
	pgschema: &str,
	current_config: &PgConfig,
	intended_config: &PgConfig,
) -> Result<String, RuntimeError> {
	#[cfg(debug_assertions)]
	let mut command = {
		let mut command = tokio::process::Command::new("uv");
		command.args("tool run -p 3.11 --with psycopg2-binary --with setuptools migra".split_whitespace());
		command
	};
	#[cfg(not(debug_assertions))]
	let mut command = tokio::process::Command::new("migra");

	let output = command
		.arg("--unsafe")
		.arg("--with-privileges")
		.arg("--schema").arg(pgschema)
		.arg(convert_db_config(current_config))
		.arg(convert_db_config(intended_config))
		.output()
		.await?;

	if !output.stderr.is_empty() {
		let e = format!("migra failed: {}\n\n{}", output.status, String::from_utf8_lossy(&output.stderr));
		return Err(RuntimeError::OtherError(e));
	}
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn convert_db_config(url: &PgConfig) -> String {
	let user = url.get_user().unwrap_or_default();
	let password = urlencoding::encode_binary(url.get_password().unwrap_or_default());
	// let host = url.get_hosts().get(0).map(|host| host.into()).unwrap_or_default();
	let host = "localhost";
	let port = url.get_ports().get(0).unwrap_or(&5432);
	let db = url.get_dbname().unwrap_or_default();
	format!("postgresql://{user}:{password}@{host}:{port}/{db}")
	// let mut url = url.to_config_lossy();
	// url.set_scheme("postgresql").unwrap();
	// url.set_query(None);
	// url.to_string()
}

// pub async fn clean_all_temp_dbs(base_config: &PgConfig) -> Result<(), postgres::Error> {
// 	let (mut client, connection) = base_config.connect(postgres::NoTls).await?;
//  tokio::spawn(async move { if let Err(e) = connection.await { log::error!("DB connection error: {}", e); } });
// 	let temp_dbs = client.query(
// 		r#"select datname from pg_database where obj_description(oid, 'pg_database') = $1;"#,
// 		TEMP_DB_COMMENT,
// 	)
// 	.await?;

// 	for row in temp_dbs {
//    let datname: String = row.get(0);
// 		if let Err(e) = client.batch_execute(&format!(r#"drop database if exists "{}";"#, datname))
// 			.await
// 		{
// 			eprintln!("Failed to drop temporary database {}: {}", row.datname, e);
// 		}
// 	}

// 	Ok(())
// }
