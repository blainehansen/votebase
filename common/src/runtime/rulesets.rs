use std::{cell::RefCell, rc::Rc};
use deno_core::OpState;
use crate::rulesets::{BundledRuleset, propose_candidate_ruleset};
use crate::runtime::RunInfo;
use crate::{PgClient};

use super::{demand_external_allowed, run_err};


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
