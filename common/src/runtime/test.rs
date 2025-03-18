use super::*;

const DEV_DB_URL: &'static str = "postgres://dev_admin_user:dev_password@localhost:5432/dev_db";
fn opt() -> PgOpt {
	DEV_DB_URL.parse().unwrap()
}

fn boil_string(s: &str) -> String {
	s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[tokio::test]
async fn test_propose_self_replacement() {
	let pool = sqlx::postgres::PgPoolOptions::new().connect(DEV_DB_URL).await.unwrap();
	sqlx::raw_sql(r#"
		drop schema if exists "ruleset:root" cascade;
		drop role if exists "role:root|migrator";
		drop role if exists "role:root|action";
		drop role if exists "role:root|view";
		delete from votebase_catalog.candidate_replacement_ruleset where true;
		delete from votebase_catalog.ruleset where true;
	"#).execute(&pool).await.unwrap();

	let current_full_path = "root";
	create_ruleset(
		&pool, None, &current_full_path, &vec![], &vec![],
		"", "create table stuff (id uuid primary key);",
	).await.unwrap();

	run_function::<()>(
		current_full_path.to_string(), r#"
			votebase.registerAction("test_action", async () => {
				const u = await votebase.proposeSelfReplacement({
					code: `
						votebase.registerAction("action1", () => {});
						votebase.registerAction("action2", () => {});
						votebase.registerView("view1", () => {});
					`,
					db_schema: "create table stuff (id uuid primary key, color text not null);",
					db_migration: "alter table stuff add column color text not null;",
				})
				if (typeof u !== 'string' || u.length !== 36)
					throw new Error(`proposeSelfReplacement didn't return uuid: ${u}`)
			})
		"#.to_string(), "test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap();

	let mut result = sqlx::query!(r#"
		select candidate_for, actions, views, code, db_schema, db_migration
		from votebase_catalog.candidate_replacement_ruleset
	"#).fetch_one(&pool).await.unwrap();

	assert_eq!(result.candidate_for, "root");
	result.actions.sort();
	assert_eq!(result.actions, &["action1", "action2"]);
	assert_eq!(result.views, &["view1"]);
	assert_eq!(boil_string(&result.code), boil_string(r#"
		votebase.registerAction("action1", () => {});
		votebase.registerAction("action2", () => {});
		votebase.registerView("view1", () => {});
	"#));
	assert_eq!(result.db_schema, "create table stuff (id uuid primary key, color text not null);");
	assert_eq!(result.db_migration, "alter table stuff add column color text not null;");


	let result = run_function::<()>(
		current_full_path.to_string(), r#"
			votebase.registerAction("test_action", async () => {
				await votebase.proposeSelfReplacement({
					code: '',
					db_schema: "create table stuff (id uuid primary key, color text not null);",
					db_migration: "alter table stuff add column color text;",
				})
			})
		"#.to_string(), "test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains("candidate for root has misdeclared schema"));
}

#[tokio::test]
async fn run_function_basics() {
	let pool = sqlx::postgres::PgPoolOptions::new().connect(DEV_DB_URL).await.unwrap();
	let result = run_function::<u32>(
		"".into(),
		r#"
			await Deno.core.ops.op_sql_execute_many("select 1")
			votebase.registerAction("test_action", async () => {
				return true
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

	let result = run_function::<u32>(
		"".into(), r#"
			votebase.registerAction("test_action", async () => {
				return await Deno.core.ops.op_sql_execute_many("select 1")
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap();
	assert_eq!(result, 1);

	let result = run_function::<u32>(
		"".into(), r#"
			await Deno.core.ops.op_sql_execute_many("select 1")
			votebase.registerView("test_view", async () => {
				return true
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, opt(), opt(), pool.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

	let result = run_function::<u32>(
		"".into(), r#"
			votebase.registerView("test_view", async () => {
				return await Deno.core.ops.op_sql_execute_many("select 1")
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, opt(), opt(), pool.clone(),
	).await.unwrap();
	assert_eq!(result, 1);
}
