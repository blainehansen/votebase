use super::*;
use crate::{deadpool, postgres};
use serde_json::Value as Val;

const DEV_DB_URL: &'static str = "postgres://dev_admin_user:dev_admin_password@localhost:5432/dev_db";
fn conf() -> PgConfig {
	DEV_DB_URL.parse().unwrap()
}

fn boil_string(s: &str) -> String {
	s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[test]
fn test_convert_db_url() {
	assert_eq!(convert_db_config(&conf()), "postgresql://dev_admin_user:dev_admin_password@localhost:5432/dev_db");
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
	let scheduled_action_queue = ScheduledActionQueue::new();
	let pool = deadpool::Pool::builder(deadpool::Manager::new(conf(), postgres::NoTls)).max_size(5).build().unwrap();
	let mut client = pool.get().await.unwrap();
	client.batch_execute(r#"
		drop schema if exists "ruleset:root" cascade;
		drop role if exists "role:root|migrator";
		drop role if exists "role:root|action";
		drop role if exists "role:root|view";
		delete from votebase_catalog.candidate_replacement_ruleset where true;
		delete from votebase_catalog.ruleset where true;
	"#).await.unwrap();

	let current_full_path = "root";
	create_ruleset(
		&conf(), &mut client, None, &current_full_path, &vec![], &vec![],
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
		"#.to_string(), "test_action", serde_json::json!(null), FnType::Action, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap();

	let mut result = queries::rulesets::test_select_candidate_replacement_ruleset()
		.bind(&client).one().await.unwrap();

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
		"#.to_string(), "test_action", serde_json::json!(null), FnType::Action, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains("candidate for root has misdeclared schema"));
}

#[tokio::test]
async fn run_function_basics() {
	let scheduled_action_queue = ScheduledActionQueue::new();
	let pool = deadpool::Pool::builder(deadpool::Manager::new(conf(), postgres::NoTls)).max_size(5).build().unwrap();
	let result = run_function::<()>(
		"".into(),
		r#"
			await votebase.sqlFetchScalar("select 1")
			votebase.Action("test_action", async () => {
				return true
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

	let result = run_function::<u32>(
		"".into(), r#"
			votebase.Action("test_action", async () => {
				const v = await votebase.sqlFetchScalar("select 1")
				if (v !== 1) throw new Error(`sqlFetchScalar didn't return number: ${v}`)
				return v
			})
		"#.to_string(),
		"test_action", serde_json::json!(null), FnType::Action, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap();
	assert_eq!(result, 1);

	let result = run_function::<()>(
		"".into(), r#"
			await votebase.sqlFetchScalar("select 1")
			votebase.View("test_view", async () => {
				return true
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap_err();
	assert!(result.to_string().contains(ERR_EXTERNAL_NOT_ALLOWED));

	// sqlExecuteStatements

	// sqlFetchScalar
	let result = run_function::<String>(
		"".into(), r#"
			votebase.View("test_view", async () => {
				const v = await votebase.sqlFetchScalar("select 'hello'")
				if (v !== 'hello') throw new Error(`sqlFetchScalar didn't return string: ${v}`)
				return v
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap();
	assert_eq!(result, "hello");

	// sqlFetchOne
	let result = run_function::<Val>(
		"".into(), r#"
			votebase.View("test_view", async () => {
				const r = await votebase.sqlFetchOne(`select 'hello' as yo, true as hmm, '[1, null, "a"]'::jsonb as arr`)
				if (r.yo !== 'hello' || r.hmm !== true || !(Array.isArray(r.arr) && r.arr[0] === 1 && r.arr[1] === null && r.arr[2] === 'a'))
					throw new Error(`sqlFetchOne didn't return proper record: ${r}`)
				return r
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap();
	if let Val::Object(o) = result {
		assert_eq!(o.get("yo").unwrap(), "hello");
		assert_eq!(o.get("hmm").unwrap(), true);
		assert_eq!(o.get("arr").unwrap(), &Val::Array(vec![1.into(), Val::Null, "a".into()]));
	} else { assert!(false, "isn't object") };

	// sqlFetchOptional
	let result = run_function::<Val>(
		"".into(), r#"
			votebase.View("test_view", async () => {
				const n = await votebase.sqlFetchOptional(`
					with a as (select 'hello' as yo, true as hmm, '[1, null, "a"]'::jsonb as arr)
					select * from a where hmm = false
				`)
				if (n !== null)
					throw new Error(`sqlFetchOptional didn't return null: ${n}`)

				const r = await votebase.sqlFetchOptional(`
					with a as (select 'hello' as yo, true as hmm, '[1, null, "a"]'::jsonb as arr)
					select * from a where hmm = true
				`)
				if (r.yo !== 'hello' || r.hmm !== true || !(Array.isArray(r.arr) && r.arr[0] === 1 && r.arr[1] === null && r.arr[2] === 'a'))
					throw new Error(`sqlFetchOptional didn't return proper record: ${r}`)
				return r
			})
		"#.to_string(),
		"test_view", serde_json::json!(null), FnType::View, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	).await.unwrap();
	if let Val::Object(o) = result {
		assert_eq!(o.get("yo").unwrap(), "hello");
		assert_eq!(o.get("hmm").unwrap(), true);
		assert_eq!(o.get("arr").unwrap(), &Val::Array(vec![1.into(), Val::Null, "a".into()]));
	} else { assert!(false, "isn't object") };

	// sqlFetchAll





	// let client = pool.get().await.unwrap();
	// queries::members::test_delete_all_members().bind(&client).await.unwrap();
	// let luke = run_function::<String>(
	// 	"".into(), r#"
	// 		votebase.Action("test_action", async () => {
	// 			const [luke, leia, vader] = await Promise.all([
	// 				votebase.enrollMember("luke@rebels.org"),
	// 				votebase.enrollMember("leia@rebels.org"),
	// 				votebase.enrollMember("vader@rebels.org"),
	// 			])
	// 			if (typeof luke !== 'string' || luke.length !== 36) throw new Error(`enrollMember didn't return uuid: ${luke}`)
	// 			if (typeof leia !== 'string' || leia.length !== 36) throw new Error(`enrollMember didn't return uuid: ${leia}`)
	// 			if (typeof vader !== 'string' || vader.length !== 36) throw new Error(`enrollMember didn't return uuid: ${vader}`)

	// 			await Promise.all([
	// 				votebase.removeMemberByEmail("vader@rebels.org"),
	// 				votebase.removeMemberByUuid(leia),
	// 			])

	// 			return luke
	// 		})
	// 	"#.to_string(),
	// 	"test_action", serde_json::json!(null), FnType::Action, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	// ).await.unwrap();
	// let result = queries::members::test_select_all_members().bind(&client).one().await.unwrap();
	// assert_eq!(result.id, luke.parse::<Uuid>().unwrap());
	// assert_eq!(result.email, "luke@rebels.org");

	// client.batch_execute(r#"
	// 	delete from votebase_catalog.detached_scheduled_action where true;
	// 	delete from votebase_catalog.ruleset where true;
	// 	insert into votebase_catalog.ruleset (
	// 		parent_full_path, "name", actions, views, code, db_schema
	// 	) values (
	// 		null, 'root', ARRAY[]::text[], ARRAY[]::text[], '', ''
	// 	) returning full_path, migrator_pass, action_pass, view_pass;
	// "#).await.unwrap();

	// let uuid = run_function::<String>(
	// 	"root".into(), r#"
	// 		const a = votebase.Action("call_action", async () => {
	// 			// do nothing
	// 		})

	// 		votebase.Action("test_action", async () => {
	// 			const d = new Date()
	// 			d.setDate(d.getDate() + 1)

	// 			const [sch1, sch2] = await Promise.all([
	// 				votebase.scheduleAction("call_action with 1", d, a, 1),
	// 				votebase.scheduleAction("call_action with 2", d, a, 2),
	// 			])
	// 			if (typeof sch1 !== 'string' || sch1.length !== 36) throw new Error(`scheduleAction didn't return uuid: ${sch1}`)
	// 			if (typeof sch2 !== 'string' || sch2.length !== 36) throw new Error(`scheduleAction didn't return uuid: ${sch2}`)

	// 			await votebase.unscheduleAction(sch2)
	// 			return sch1
	// 		})
	// 	"#.to_string(),
	// 	"test_action", serde_json::json!(null), FnType::Action, conf(), conf(), pool.clone(), scheduled_action_queue.clone(),
	// ).await.unwrap();
	// let result = queries::scheduled::test_select_detached_scheduled_actions().bind(&client).one().await.unwrap();
	// assert_eq!(result.id, uuid.parse::<Uuid>().unwrap());
	// assert_eq!(result.description, "call_action with 1");
	// assert_eq!(result.scheduled_time.date_naive(), chrono::Utc::now().date_naive() + ::chrono::Days::new(1));
	// assert_eq!(result.full_path, "root");
	// assert_eq!(result.action_name, "call_action");
	// assert_eq!(result.action_arg, serde_json::json!(1));

	// // // TODO find a way to test the real thing now that scheduleAction will actually queue a tokio task
	// // assert!(false);
}
