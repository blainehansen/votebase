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

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
	#[error("the migration for `{0}` doesn't match the provided schema")]
	InvalidMigration(String),
	#[error("the typescript code has errors\n\n{0}")]
	InvalidTs(String),
	#[error(transparent)]
	Container(#[from] temp_container_utils::ContainerError),
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Db(#[from] postgres::Error),
}

// it looks to make like this function should work correctly for both new creations and replacements
async fn validate_bundled_ruleset(
	full_path: &str,
	bundled_ruleset: &BundledRuleset,
	// this server_db_archive should be in a server wide mutex, since we're checking against it and don't want anyone to change it while we're operating
	server_db_archive_path: &std::path::Path,
) -> Result<(), ValidationError> {

	// ### bundled_ruleset.code
	// place the typescript code in a temp directory and check it
	let temp_dir = tmpdir::TmpDir::new("validate_bundled_ruleset").await?;
	let ts_file = temp_dir.as_ref().join("ruleset.ts");
	tokio::fs::write(&ts_file, &bundled_ruleset.code).await?;
	podman_votebase_tsc(temp_dir.as_ref()).await?;

	// TODO also actually *run* the ruleset code to gather its fns and make sure they don't overlap, so Runtime considerations

	// check that the migration is correct
	// this is complex. we need to load *enough* of the existing server schema to be able to just create two databases:
	// one with the current state (including the state of the ruleset this one is replacing, if that's the case), and execute the bundled_ruleset.db_migration on it
	// and another where we also do the existing server schema *but without the schema of the target ruleset*, but then we also execute the bundled_ruleset.db_schema
	// for now we're using the entire thing! except for the "declarative" version that needs to exclude the target schema
	// pg_dump --exclude-schema=example -f output_file.sql,
	// at the end of this process we should have two databases that should be identical
	// we want the archive target file to have a known location (probably something named after the overall dbname), and whenever the schema changes (which only happens on ruleset changes!) we acquire a server wide mutex
	let formatted_ruleset_schema = format_ruleset_schema(full_path);

	temp_container_utils::with_temp_postgres_client(async |db_container_name, mut config, client| {
		// ### bundled_ruleset.db_schema
		// ### bundled_ruleset.db_migration
		let db_user = config.get_user().unwrap().to_string();

		let from_db_name = "tempdb|from";
		let to_db_name = "tempdb|to";
		client.batch_execute(&format!(r#"create database "{from_db_name}""#)).await?;
		client.batch_execute(&format!(r#"create database "{to_db_name}""#)).await?;

		let from_config = { let mut config = config.clone(); config.dbname(from_db_name); config };
		let to_config = { config.dbname(to_db_name); config };
		let (from_client, from_connection) = from_config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = from_connection.await { log::error!("DB connection error: {}", e); } });
		let (to_client, to_connection) = to_config.connect(postgres::NoTls).await?;
		tokio::spawn(async move { if let Err(e) = to_connection.await { log::error!("DB connection error: {}", e); } });


		podman_pg_restore(&db_container_name, &from_db_name, &db_user, server_db_archive_path, Some(&formatted_ruleset_schema)).await?;
		// TODO create_ruleset?
		from_client.batch_execute(&bundled_ruleset.db_schema).await?;

		podman_pg_restore(&db_container_name, &to_db_name, &db_user, server_db_archive_path, None).await?;
		to_client.batch_execute(&bundled_ruleset.db_migration).await?;

		let diff = podman_compute_diff(&db_container_name, &formatted_ruleset_schema, &from_config, &to_config).await?;
		if !diff.is_empty() {
			// log::error!("{}", diff);
			return Err(ValidationError::InvalidMigration(full_path.to_string()))
		}

		// here we go through all of these and actually *execute* all the implied changes to children!
		// - we make sure all the "keep" actually exist
		// - we make sure all the "replace" actually exist, and that they recursively make sense (have to be smart, we can't just call validate_bundled_ruleset again! we need to chop up this functionality so that we have a root call that starts the podman postgres and the others just perform actions)
		// - we make sure that after our modifications and deletions everything still makes sense. this last one is something that probably happens only once at the top level, and includes checking our own db_uses
		bundled_ruleset.static_children;
		// these are easy, we just have to make sure they point to real actions that are in *this* ruleset (no references here), and that they're well-formed, which I'm pretty sure will have already happened at parse time
		bundled_ruleset.static_recurring_events;

		// TODO check that all the db_uses still make sense, along with validating that everything *else* in the system hasn't had uses orphaned
		bundled_ruleset.db_uses;
		Ok(())
	}).await??;


	// we've established with experiments that you can indeed delete objects, it seems of any kind (certainly functions and tables) that are relied on by functions, and now those functions will throw errors when you call them
	// this means I have no choice but to implement the `uses` system. right now to cut scope I think I shouldn't do the adjoining "allows" system, but instead just allow anything to reference anything (if it isn't in the catalog)
	// this means I need a complete system to be able to point to objects to say you reference them
	// the easy version of this is to just allow pointing to the object itself, you only need a ruleset_path and an object name, along with the "use kind", basically TableReference | TableQueryAndReference | Type | ViewFunction | ActionFunction, and since you aren't narrowing to specific columns
	// you have to narrow to specific columns, because the uses section has to specify enough type information about these objects that you can create stand-ins from scratch!
	// this means we need some micro-language, either parsed or declared fully as a serde-able ast, that describes these permissions
	// in the context of a json file they'd look something like this:
	// "~v1": "ReferenceTable ruleset_path.table_name(c1 t1, c2)" // https://docs.rs/sqlparser/latest/sqlparser/ast/struct.TableAlias.html
	// "~v2": "QueryAndReferenceTable p.t(c1, c2)"
	// "~v3": "UseType p.t"
	// "~v3": "CallViewFunction p.t(p1 t1, p2 t2) -> tr"
	// "~v3": "CallActionFunction p.t(p1 t1, p2 t2) -> tr"
	// https://www.postgresql.org/docs/current/catalog-pg-proc.html

	// in order to check whether a deletion or replacment is valid, we need to just perform the intended changes on the database, and then reflect the new schema (probably being smart to narrow to only the affected rulesets), and then go through the `uses` of *all* the other rulesets that mention the affected ones, and checking if they all still exist and we haven't severed anything
	// oh importantly, the `uses` only needs the permission and the structure, and the var name is good enough (we obviously aren't "qualifying" a vague concept of a thing!)
	// it's the actual "fulfilling" vars declarations that need to be fully qualified


	// TODO and also run against the ruleset graph! so we need to see if this is a valid replacement for what existed before.
	// what constitutes that? wherever a 'keep' is issued there actually has to be something that existed before. and wherever we're replacing something we need to be able to recursively check that replacement is valid. and wherever we're deleting we need to check recursively that those deletions are okay
	// TODO now do any checking for "uses" violations. and is that it?

	Ok(())
}

fn validate_schema_uses(
	// a mapping of rulesets to the objects they use
	possibly_effected_objects: HashMap<String, Vec<Usage>>,
	// a list of all the possibly relevant (and allowed!) objects after all implied changes have been made
	new_present_objects: Vec<UseableObject>,
) -> Result<(), ValidationError> {
	unimplemented!()
}

struct Usage {
	ruleset_path: String,
	object_name: String,
	usage_kind: UsageKind,
}

enum UsageKind {
	Type,
	Table { can_query: bool, columns: Vec<(String, String)> },
	Function { is_action: bool, params: Vec<String>, return_type: String },
}

struct UseableObject {
	ruleset_path: String,
	object_name: String,
	db_object: UseableDbObject,
}

enum UseableDbObject {
	Type,
	Table { can_query: bool, columns: Vec<FullColumn> },
	Function { is_action: bool, params: Vec<FullParam>, return_type: PgType },
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

// this full_ruleset_dir will probably be a temp directory in the server context
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
	exclude_schema: Option<&str>,
) -> std::io::Result<()> {
	// TODO okay to achieve this, we need to change it so the postgres container that's running has a volume to some directory we can read and write
	// then we use podman *exec* to run pg_restore/pg_dump inside the container
	// we actually might not need to bother with the volumes if instead we instead receive stdout into a file in tokio and input to stdin from a file

	// pg_dump outputs to stdout if no --file argument is given, and pg_restore reads from stdin if no --file is given
	// podman exec db_container_name pg_dump -d db_name -U db_user -f /whatever_volume_name/server_db_archive_path

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

	if let Some(exclude_schema) = exclude_schema {
		// --exclude-schema=pattern
		command.arg("-N").arg(exclude_schema);
	}

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

// 	let output = temp_container_utils::podman_run(
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
