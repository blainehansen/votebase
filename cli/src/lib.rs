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


#[derive(Debug, serde::Deserialize)]
struct AbstractRuleset {
	// this comes in the source files, not the bundle
	// /// Typescript code containing all the Actions and Views of the `Ruleset`.
	// /// This must be fully bundled, meaning it's the entire codebase of the whole `Ruleset`, including the queries code etc.
	// /// It doesn't need to include the votebase runtime.
	// code: String,

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
