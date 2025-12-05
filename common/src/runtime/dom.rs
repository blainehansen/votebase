// op_fetch: (url: string) => Promise<string>,
#[deno_core::op2(async)]
#[string]
pub async fn op_fetch(
	state: Rc<RefCell<OpState>>,
	#[string] url: String,
) -> Result<String, deno_error::JsErrorBox> {
	demand_external_allowed(state.as_ref())?;

	let body = reqwest::get(url).await.map_err(js_err)?.text().await.map_err(js_err)?;
	Ok(body)
}

// #[deno_core::op2(async)]
// async fn op_set_timeout(
// 	state: Rc<RefCell<OpState>>,
// 	delay: f64
// ) -> Result<(), deno_error::JsErrorBox> {
// 	demand_external_allowed(state.as_ref())?;
// 	// TODO pretty important to limit these timeouts
// 	// perhaps don't even allow this? all asynchrony in votebase should occur through scheduled events?

// 	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
// 	Ok(())
// }
