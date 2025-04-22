use super::*;

const DEV_DB_URL: &'static str = "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db";
fn opt() -> PgOpt {
	DEV_DB_URL.parse().unwrap()
}

fn boil_string(s: &str) -> String {
	s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[test]
fn test_convert_db_url() {
	assert_eq!(convert_db_url(&opt()), "postgresql://dev_admin_user:dev%5Fadmin%5Fpassword@localhost:5432/dev_db");
}

#[tokio::test(start_paused = true)]
async fn test_compute_time_until() {
	use ::chrono::TimeDelta;
	use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

	let now = chrono::Utc::now();
	let should_complete_time = compute_time_until(now + TimeDelta::days(2));
	let should_not_complete_time = compute_time_until(now + TimeDelta::days(2) + TimeDelta::milliseconds(2));

	let should_complete = Arc::new(AtomicBool::new(false));
	tokio::spawn({
		let should_complete = should_complete.clone();
		async move {
			tokio::time::sleep_until(should_complete_time).await;
			should_complete.store(true, Ordering::Relaxed);
		}
	});

	let should_not_complete = Arc::new(AtomicBool::new(false));
	tokio::spawn({
		let should_not_complete = should_not_complete.clone();
		async move {
			tokio::time::sleep_until(should_not_complete_time).await;
			should_not_complete.store(true, Ordering::Relaxed);
		}
	});

	tokio::time::sleep((TimeDelta::days(2) + TimeDelta::milliseconds(1)).to_std().unwrap()).await;
	assert!(should_complete.load(Ordering::Relaxed));
	assert!(!should_not_complete.load(Ordering::Relaxed));
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
			votebase.Action("test_action", async () => {
				const u = await votebase.proposeSelfReplacement({
					code: `
						votebase.Action("action1", () => {});
						votebase.Action("action2", () => {});
						votebase.View("view1", () => {});
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
		votebase.Action("action1", () => {});
		votebase.Action("action2", () => {});
		votebase.View("view1", () => {});
	"#));
	assert_eq!(result.db_schema, "create table stuff (id uuid primary key, color text not null);");
	assert_eq!(result.db_migration, "alter table stuff add column color text not null;");


	let result = run_function::<()>(
		current_full_path.to_string(), r#"
			votebase.Action("test_action", async () => {
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
			await votebase.sqlFetchScalar("select 1")
			votebase.Action("test_action", async () => {
				return true
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

	let result = run_function::<u32>(
		"".into(), r#"
			votebase.Action("test_action", async () => {
				const v = await votebase.sqlFetchScalar("select 1")
				console.log(v)
				return v
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap();
	assert_eq!(result, 1);

	let result = run_function::<u32>(
		"".into(), r#"
			await votebase.sqlFetchScalar("select 1")
			votebase.View("test_view", async () => {
				return true
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, opt(), opt(), pool.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

	let result = run_function::<u32>(
		"".into(), r#"
			votebase.View("test_view", async () => {
				return await votebase.sqlFetchScalar("select 1")
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, opt(), opt(), pool.clone(),
	).await.unwrap();
	assert_eq!(result, 1);

	sqlx::query!(r#"delete from votebase_catalog.member where true;"#).execute(&pool).await.unwrap();
	let luke = run_function::<String>(
		"".into(), r#"
			votebase.Action("test_action", async () => {
				const [luke, leia, vader] = await Promise.all([
					votebase.enrollMember("luke@rebels.org"),
					votebase.enrollMember("leia@rebels.org"),
					votebase.enrollMember("vader@rebels.org"),
				])
				if (typeof luke !== 'string' || luke.length !== 36) throw new Error(`enrollMember didn't return uuid: ${luke}`)
				if (typeof leia !== 'string' || leia.length !== 36) throw new Error(`enrollMember didn't return uuid: ${leia}`)
				if (typeof vader !== 'string' || vader.length !== 36) throw new Error(`enrollMember didn't return uuid: ${vader}`)

				await Promise.all([
					votebase.removeMemberByEmail("vader@rebels.org"),
					votebase.removeMemberByUuid(leia),
				])

				return luke
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap();
	let result = sqlx::query!(r#"select id, email from votebase_catalog.member"#).fetch_one(&pool).await.unwrap();
	assert_eq!(result.id, luke.parse::<sqlx::types::Uuid>().unwrap());
	assert_eq!(result.email, "luke@rebels.org");

	sqlx::raw_sql(r#"
		delete from votebase_catalog.detached_scheduled_action where true;
		delete from votebase_catalog.ruleset where true;
		insert into votebase_catalog.ruleset (
			parent_full_path, "name", actions, views, code, db_schema
		) values (
			null, 'root', ARRAY[]::text[], ARRAY[]::text[], '', ''
		) returning full_path, migrator_pass, action_pass, view_pass;
	"#).execute(&pool).await.unwrap();
	let uuid = run_function::<String>(
		"root".into(), r#"
			const a = votebase.Action("call_action", async () => {
				// do nothing
			})

			votebase.Action("test_action", async () => {
				const d = new Date()
				d.setDate(d.getDate() + 1)

				const [sch1, sch2] = await Promise.all([
					votebase.scheduleAction("call_action with 1", d, a, 1),
					votebase.scheduleAction("call_action with 2", d, a, 2),
				])
				if (typeof sch1 !== 'string' || sch1.length !== 36) throw new Error(`scheduleAction didn't return uuid: ${sch1}`)
				if (typeof sch2 !== 'string' || sch2.length !== 36) throw new Error(`scheduleAction didn't return uuid: ${sch2}`)

				await votebase.unscheduleAction(sch2)
				return sch1
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, opt(), opt(), pool.clone(),
	).await.unwrap();
	let result = sqlx::query!(r#"
		select id, description, scheduled_time, full_path, action_name, action_arg
		from votebase_catalog.detached_scheduled_action
	"#).fetch_one(&pool).await.unwrap();
	assert_eq!(result.id, uuid.parse::<sqlx::types::Uuid>().unwrap());
	assert_eq!(result.description, "call_action with 1");
	assert_eq!(result.scheduled_time.date_naive(), chrono::Utc::now().date_naive() + ::chrono::Days::new(1));
	assert_eq!(result.full_path, "root");
	assert_eq!(result.action_name, "call_action");
	assert_eq!(result.action_arg, serde_json::json!(1));

	// TODO find a way to test the real thing now that scheduleAction will actually queue a tokio task
	assert!(false);
}
