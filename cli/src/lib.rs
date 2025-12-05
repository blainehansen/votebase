mod cmd_init;
pub use cmd_init::cmd_init;

mod cmd_dev_check;
pub use cmd_dev_check::{cmd_dev, cmd_check};

mod cmd_generate_migration;
pub use cmd_generate_migration::cmd_generate_migration;

// TODO need to test situations where rulesets are incorrect, either for structural but especially permissions reasons

// bad_ts the ruleset file itself is malformed. honestly this probably should be mostly done in a series of typescript tests etc

// both of these are very important, they're the places where we'll do things like check that the permissions system is working correctly, and that the schema naming and relationships make sense
// but, this should probably be tested at the level of run_sql_checking_and_generation rather than here
// check:
// - we can't really tell the difference between an action and a query.... hmmm. the cli should probably have a "test" command that runs the real ruleset fns on randomly generated input, which we'll be able to do if they declare an input schema!
// - the schema isn't trying to "reach outside" of itself to modify other schemas or set its own search path etc. it isn't trying to modify the catalog, or create objects it isn't allowed to create

pub fn cmd_create_bundle_info() -> anyhow::Result<()> {
	Ok(())
}

pub async fn cmd_fetch_server_schema() -> anyhow::Result<()> {
	Ok(())
	//
}

mod cmd_bundle;
pub use cmd_bundle::cmd_bundle;
