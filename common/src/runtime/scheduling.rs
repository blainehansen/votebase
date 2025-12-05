use std::{cell::RefCell, rc::Rc};
use deno_core::OpState;
use uuid::Uuid;
use log::info;

use super::{RuntimeError, run_err, run_action, demand_external_allowed, ServerPgConfig};
use crate::{PgPool, PgConfig, queries, ScheduledActionKind};

#[derive(Clone)]
pub struct ScheduledActionQueue {
	send: tokio::sync::mpsc::UnboundedSender<ScheduledAction>,
}

type ScheduledAction = (PgPool, PgConfig, ScheduledActionKind, Uuid, tokio::time::Instant);

impl ScheduledActionQueue {
	pub fn new() -> Self {
		let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
		let spawner = Self { send };

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.unwrap();

		let thread_spawner = spawner.clone();
		std::thread::spawn(move || {
			let local = tokio::task::LocalSet::new();

			local.spawn_local(async move {
				while let Some(scheduled_action) = recv.recv().await {
					tokio::task::spawn_local(do_scheduled_action(scheduled_action, thread_spawner.clone()));
				}
			});

			rt.block_on(local);
		});

		spawner
	}

	pub fn queue(
		&self,
		server_role_pool: PgPool,
		server_pg_config: PgConfig,
		scheduled_action_kind: ScheduledActionKind,
		scheduled_action_uuid: Uuid,
		scheduled_time: chrono::DateTime<chrono::Utc>,
	// ) -> Result<(), tokio::sync::mpsc::error::SendError<ScheduledAction>> {
	) {
		let scheduled_time = compute_time_until(scheduled_time);
		let scheduled_action = (
			server_role_pool, server_pg_config,
			scheduled_action_kind, scheduled_action_uuid, scheduled_time,
		);
		if let Err(e) = self.send.send(scheduled_action) {
			log::error!("couldn't send scheduled action? {}", e);
		};
	}
}

fn compute_time_until(scheduled_time: chrono::DateTime<chrono::Utc>) -> tokio::time::Instant {
	let duration_until = scheduled_time.signed_duration_since(chrono::Utc::now()).to_std().unwrap();
	tokio::time::Instant::now() + duration_until
}

async fn do_scheduled_action(scheduled_action: ScheduledAction, spawner: ScheduledActionQueue) {
	let (
		server_role_pool, server_pg_config,
		scheduled_action_kind, scheduled_action_uuid, scheduled_time,
	) = scheduled_action;

	tokio::time::sleep_until(scheduled_time).await;

	log::info!("attempting action {} of type {}", scheduled_action_uuid, &scheduled_action_kind);
	let result = match scheduled_action_kind {
		// ScheduledActionKind::Recurring => {
		// 	execute_recurring_action(server_role_pool, server_pg_config, false, scheduled_action_uuid).await
		// },
		ScheduledActionKind::DetachedRecurring => {
			execute_recurring_action(&spawner, server_role_pool, server_pg_config, scheduled_action_uuid).await
		},
		ScheduledActionKind::DetachedScheduled => {
			execute_scheduled_action(spawner.clone(), server_role_pool, server_pg_config, scheduled_action_uuid).await
			// if let Err(e) = result {
			// 	error!("failed to execute scheduled action {}; {}", scheduled_action_uuid, e);
			// }
		},
	};

	if let Err(e) = result {
		log::error!("error when queueing scheduled action {}: {}", scheduled_action_uuid, e);
	}
}


pub async fn execute_recurring_action(
	scheduled_action_queue: &ScheduledActionQueue,
	server_role_pool: PgPool,
	server_pg_config: PgConfig,
	// is_detached: bool,
	scheduled_action_uuid: uuid::Uuid,
) -> Result<(), RuntimeError> {
	// let scheduled_action_kind = if is_detached { ScheduledActionKind::DetachedRecurring } else { ScheduledActionKind::Recurring };
	let scheduled_action_kind = ScheduledActionKind::DetachedRecurring;

	let client = server_role_pool.get().await?;
	let action = queries::scheduled::acquire_detached_recurring_action()
		.bind(&client, &scheduled_action_uuid).opt().await?;
	// if is_detached {
	// } else {
	// 	unimplemented!()
	// };

	match action {
		None => { info!("wasn't able to acquire scheduled action {}", scheduled_action_uuid); Ok(()) },
		Some(action) => {
			let scheduled = action.next_scheduled_time.and_utc();
			let now = chrono::Utc::now();
			log::info!("{} scheduled: {}; actual: {}; {}", scheduled_action_uuid, scheduled, now, action.description);
			if scheduled > now {
				log::info!("{} not doing it yet", scheduled_action_uuid);
				scheduled_action_queue.queue(server_role_pool, server_pg_config, scheduled_action_kind, scheduled_action_uuid, scheduled);
				return Ok(())
			}

			run_action(
				action.full_path, action.code, &action.action_name, &action.action_pass, &action.migrator_pass, action.arg,
				server_pg_config.clone(), &server_role_pool, scheduled_action_queue.clone(),
			).await?;

			let next_scheduled_time = queries::scheduled::release_detached_recurring_action()
				.bind(&client, &scheduled_action_uuid).one().await?.and_utc();
			// if is_detached {
			// } else {
			// 	unimplemented!()
				// sqlx::query!(r#"
				// 	update votebase_catalog.recurring_action
				// 	set executing = false, executed_count = executed_count + 1
				// 	where id = $1
				// 	returning next_scheduled_time;
				// "#, &scheduled_action_uuid).fetch_one(&server_role_pool).await?.next_scheduled_time.and_utc()
			// };

			scheduled_action_queue.queue(server_role_pool, server_pg_config, scheduled_action_kind, scheduled_action_uuid, next_scheduled_time);

			Ok(())
		},
	}
}

pub async fn execute_scheduled_action(
	scheduled_action_queue: ScheduledActionQueue,
	server_role_pool: PgPool,
	server_pg_config: PgConfig,
	scheduled_action_uuid: uuid::Uuid,
) -> Result<(), RuntimeError> {
	let client = server_role_pool.get().await?;
	let action = queries::scheduled::acquire_detached_scheduled_action()
		.bind(&client, &scheduled_action_uuid)
		.opt().await?;

	match action {
		None => { info!("wasn't able to acquire scheduled action {}", scheduled_action_uuid); Ok(()) },
		Some(action) => {
			let scheduled = action.scheduled_time.to_utc();
			let now = chrono::Utc::now();
			let diff = scheduled - now;
			log::info!("{} ({}): scheduled: {}; actual: {}; difference: {}", scheduled_action_uuid, action.description, scheduled, now, diff);

			run_action(
				action.full_path, action.code, &action.action_name, &action.action_pass, &action.migrator_pass, action.arg,
				server_pg_config.clone(), &server_role_pool, scheduled_action_queue,
			).await?;

			queries::scheduled::remove_detached_scheduled_action()
				.bind(&client, &scheduled_action_uuid).await?;

			Ok(())
		},
	}
}



// // // op_create_recurring_action: (description: string, start: string, recurrenceGranularity: RecurrenceGranularity, recurrenceMultiplier: number, action_name: string, action_arg: JsonValue) => Promise<string>,
// #[deno_core::op2(async)]
// #[string]
// async fn op_create_recurring_action(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] description: String,
// 	#[serde] start: chrono::DateTime<chrono::Utc>,
// 	#[serde] recurrence_granularity: crate::db_types::votebase_catalog::GranularityEnum,
// 	recurrence_multiplier: i16,
// 	#[string] action_name: String,
// 	#[serde] action_arg: serde_json::Value,
// ) -> Result<String, deno_error::JsErrorBox> {
// 	demand_external_allowed(state.as_ref())?;
// 	let state = state.as_ref().borrow();
// 	let server_pg_config = state.borrow::<ServerPgConfig>().0.clone();
// 	let server_role_pool = state.borrow::<PgPool>();
// 	let scheduled_action_queue = state.borrow::<ScheduledActionQueue>();
// 	let current_full_path = state.borrow::<String>();

// 	let client = server_role_pool.get().await.map_err(run_err)?;
// 	let action = queries::scheduled::create_detached_recurring_action()
// 		.bind(&client, &current_full_path, &description, &start.naive_utc(), &recurrence_granularity, &recurrence_multiplier, &action_name, &action_arg)
// 		.one().await.map_err(run_err)?;

// 	scheduled_action_queue.queue(
// 		server_role_pool.clone(), server_pg_config,
// 		ScheduledActionKind::DetachedRecurring, action.id, action.next_scheduled_time.and_utc(),
// 	);

// 	Ok(action.id.to_string())
// }

// // op_remove_recurring_action: (uuid: string) => Promise<void>,
// #[deno_core::op2(async)]
// #[string]
// async fn op_remove_recurring_action(
// 	state: Rc<RefCell<OpState>>,
// 	#[serde] scheduled_action_uuid: Uuid,
// ) -> Result<(), deno_error::JsErrorBox> {
// 	demand_external_allowed(state.as_ref())?;
// 	let state = state.as_ref().borrow();
// 	let server_role_pool = state.borrow::<PgPool>();

// 	let client = server_role_pool.get().await.map_err(run_err)?;
// 	queries::scheduled::remove_detached_recurring_action()
// 		.bind(&client, &scheduled_action_uuid).await.map_err(run_err)?;

// 	Ok(())
// }


// // op_schedule_action: (description: string, scheduled_time: string, action_name: string, action_arg: JsonValue) => Promise<string>,
// #[deno_core::op2(async)]
// #[string]
// async fn op_schedule_action(
// 	state: Rc<RefCell<OpState>>,
// 	#[string] description: String,
// 	#[serde] scheduled_time: chrono::DateTime<chrono::Utc>,
// 	#[string] action_name: String,
// 	#[serde] action_arg: serde_json::Value,
// ) -> Result<String, deno_error::JsErrorBox> {
// 	demand_external_allowed(state.as_ref())?;
// 	let state = state.as_ref().borrow();
// 	let server_role_pool = state.borrow::<PgPool>();
// 	let server_pg_config = state.borrow::<ServerPgConfig>().0.clone();
// 	let scheduled_action_queue = state.borrow::<ScheduledActionQueue>();
// 	let current_full_path = state.borrow::<String>();

// 	let client = server_role_pool.get().await.map_err(run_err)?;
// 	let id = queries::scheduled::create_detached_scheduled_action()
// 		.bind(&client, &current_full_path, &description, &scheduled_time.fixed_offset(), &action_name, &action_arg)
// 		.one().await.map_err(run_err)?;

// 	scheduled_action_queue.queue(
// 		server_role_pool.clone(), server_pg_config,
// 		ScheduledActionKind::DetachedScheduled, id, scheduled_time,
// 	);

// 	Ok(id.to_string())
// }

// // op_unschedule_action: (uuid: string) => Promise<void>,
// #[deno_core::op2(async)]
// #[string]
// async fn op_unschedule_action(
// 	state: Rc<RefCell<OpState>>,
// 	#[serde] scheduled_action_uuid: Uuid,
// ) -> Result<(), deno_error::JsErrorBox> {
// 	demand_external_allowed(state.as_ref())?;
// 	let state = state.as_ref().borrow();
// 	let server_role_pool = state.borrow::<PgPool>();

// 	let client = server_role_pool.get().await.map_err(run_err)?;
// 	queries::scheduled::remove_detached_scheduled_action()
// 		.bind(&client, &scheduled_action_uuid).await.map_err(run_err)?;

// 	Ok(())
// }
