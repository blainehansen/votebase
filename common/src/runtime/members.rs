use std::{cell::RefCell, rc::Rc};
use deno_core::OpState;
use uuid::Uuid;

use super::{demand_external_allowed, run_err};
use crate::{FnServerPg, PgClient, queries};

// op_enroll_member: (email: string) => Promise<string>,
#[deno_core::op2(async)]
#[string]
pub async fn op_enroll_member(
	state: Rc<RefCell<OpState>>,
	#[string] email: String,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(&state.as_ref().borrow())?;
	let mut state = state.borrow_mut();
	let server_role_pg = state.borrow_mut::<FnServerPg>();
	let server_role_client = server_role_pg.get_client().await.map_err(run_err)?;

	let id = queries::members::enroll_member()
		.bind(server_role_client, &email)
		.one().await.map_err(run_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(id.to_string())
}

// op_remove_member_by_email: (email: string) => Promise<void>,
#[deno_core::op2(async)]
pub async fn op_remove_member_by_email(
	state: Rc<RefCell<OpState>>,
	#[string] email: String,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(&state.as_ref().borrow())?;
	let state = state.as_ref().borrow();
	let server_role_client = state.borrow::<PgClient>();
	queries::members::remove_member_by_email()
		.bind(server_role_client, &email).await.map_err(run_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(())
}
// op_remove_member_by_uuid: (uuid: string) => Promise<void>,
#[deno_core::op2(async)]
pub async fn op_remove_member_by_uuid(
	state: Rc<RefCell<OpState>>,
	#[serde] member_uuid: Uuid,
) -> Result<(), deno_error::JsErrorBox> {
	demand_external_allowed(&state.as_ref().borrow())?;
	let state = state.as_ref().borrow();
	let server_role_client = state.borrow::<PgClient>();
	queries::members::remove_member_by_uuid()
		.bind(server_role_client, &member_uuid).await.map_err(run_err)?;

	// TODO perhaps at some point there's a notification email sent to this person here or something

	Ok(())
}
