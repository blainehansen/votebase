# when a new ruleset is created:
- a schema is created with name `"ruleset:{full_path}"`
- a role is created for each `fn_type` to be used by that `"role:{full_path}|{fn_type}"`
  - I actually think these two have to have a cryptographically secure name, which we have to store in the database to remember
  - we need to ensure that the view role can't modify data, and the action role can't modify database objects.
  - the dangerous part of all of this is the `db_migration` we allow. we need a safe role that is given ddl abilities
  - also I think the only way to prevent the creation of `security definer` functions by the migrator role is to check the string to see if it includes them!
  - the really tricky part is how to think about *child* privileges. it totally makes sense to allow a parent ruleset to define functions that allow its children
- I think in general we need to assume good intent here? is this even possible?
- we can always safely ensure that view roles can't modify the database even in the presence of `security definer` functions if views can only read tables and read "views" (the postgres kind). this means modification is only possible through modifiable views, which have to be used in an explicitly modifying way. then we can just allow security definer functions using a migrator role
- I think this means we also need to create a role `"role:{full_path}|migrator"`
- within a ruleset, permissions are managed by the ruleset itself, and the member id system (using exclusively magic links), because it's just to insane and complicated to allow them to create roles
- in all above strings, `|` is included

```sql
-- views
-- grant connection to database
grant connect on database dev_db to "role:full_path|view";
-- grant schema usage
grant usage on schema "ruleset:{full_path}" to "role:full_path|view";

-- grant read access to future tables and views
alter default privileges in schema target_schema
grant select on tables to "role:full_path|view";
```

```sql
-- actions
-- grant connection to database
grant connect on database dev_db to "role:full_path|action";
-- grant schema usage
grant usage on schema "ruleset:{full_path}" to "role:full_path|action";

-- grant permissions for future tables
alter default privileges in schema "ruleset:{full_path}"
grant select, insert, update, delete on tables to "role:full_path|action";

-- grant for future sequences
alter default privileges in schema "ruleset:{full_path}"
grant usage on sequences to "role:full_path|action";

alter default privileges in schema "ruleset:{full_path}"
grant execute on functions to "role:full_path|action";
```

```sql
-- migrator
-- grant connection to database
grant connect on database dev_db to "role:full_path|migrator";

-- grant schema creation and usage
grant create, usage on schema "ruleset:{full_path}" to "role:full_path|migrator";

-- grant table management permissions
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on tables to "role:full_path|migrator";

-- grant sequence management
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on sequences to "role:full_path|migrator";

-- grant function management
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on functions to "role:full_path|migrator";

-- grant type management
alter default privileges in schema "ruleset:{full_path}"
grant all privileges on types to "role:full_path|migrator";
```




```
STABLE and IMMUTABLE Functions
STABLE and IMMUTABLE functions are guaranteed not to modify the database. STABLE functions return consistent results within a single table scan, while IMMUTABLE functions always return the same result for the same inputs.


-- Grant execute permission on all current functions
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO role_name;

-- Set default privileges for future functions
ALTER DEFAULT PRIVILEGES IN SCHEMA public
GRANT EXECUTE ON FUNCTIONS TO role_name;


Important Security Notes
Even with EXECUTE privileges, functions will still run with the permissions of the executing role, not the function owner. This means the role will need appropriate underlying permissions to access any data the functions read.



-- Revoke function creation permission
REVOKE CREATE ON SCHEMA public FROM role_name;

-- If needed, grant regular function creation while preventing SECURITY DEFINER
GRANT CREATE ON SCHEMA public TO role_name;
ALTER DEFAULT PRIVILEGES FOR ROLE role_name
    REVOKE ALL ON FUNCTIONS FROM PUBLIC;

```




https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#examples

the simplest way to allow bootstrapping a server is for the initial schema creation to *itself* bootstrap a root ruleset with a single action that merely accepts a proposal replacement ruleset, and if that proposal checks as well-founded it is immediately instituted
this way no special server permissions or sql templating are needed, everything happens in the logic of the server
or perhaps it makes more sense to do this at server startup? it checks to see if there is a root ruleset, and if not it inserts

# phases
- standalone rulesets that can only replace themselves
  - X view route
  - X action route (without migration)
  - X ruleset replacement proposal (in runtime)
    - X ruleset migration checking
  - ruleset replacement enaction
    - X ruleset migrations
    - action role
    - view role
  - events
    - recurring declared in ruleset itself
    - one off
  - safe fns with schemars and zod jsonschema
  - sql runtime functions https://docs.rs/sqlx/latest/sqlx/trait.Executor.html
    - fetch many (queryRows)
    - fetch optional (queryMaybeOne)
    - fetch one (queryOne)
    - execute
    - raw sql https://docs.rs/sqlx/latest/sqlx/fn.raw_sql.html
    - query maybe scalar https://docs.rs/sqlx/latest/sqlx/fn.query_scalar.html
    - query scalar https://docs.rs/sqlx/latest/sqlx/fn.query_scalar.html
    - query rows https://docs.rs/sqlx/latest/sqlx/fn.query_with.html
- client site
  - nuxt static generated?
  - login/auth nonsense
  - async list views
  - async call and display view html (views are responsible for rendering controls to call actions)
- could release?

- parent and child rulesets, but without any communication or data sharing
  - new child ruleset proposal and enaction, child ruleset replacement proposal and enaction
- could release?

- full featured `fetch`?
  - more or less just copy how deno does it? or just implement the spec using something simple like reqwest?
- parent and child rulesets, with function calling for all communication (simpler?)
  - client site display rulesets one is a member of
- parent and child rulesets, with function calling and permissions data sharing (this might actually be simpler, since it can just be a templated sql string to run over the child ruleset's roles. especially given that a parent ruleset can specify safe functions and then just allow the child to call them, and have read access to things like weight tables, this is actually probably much simpler)


how much can even be achieved with parent and child systems without communication? full persistent



// https://github.com/denoland/deno_core/issues/515
// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1201661871959310346
// https://gist.github.com/alshdavid/c9e5bc0d794e3ec9dba6afaa689b704e#file-main-rs-L51

// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1074150763460313128




- postgres table to schedule or cancel one-off events
  - runtime function for actions to schedule them
- postgres table to hold static recurring events
  - runtime function fo server to call at startup for existing and when rulset instituted
- runtime function to propose self replacement ruleset (actions returning a string means replace that ruleset)
- runtime function to propose new child ruleset
- runtime function to institute new child ruleset

- runtime function to send well-typed message (using jsonschema) to parent or child ruleset?
  - when parent institutes child ruleset, all messages that can be passed up and down and their corresponding actions are declared?


Rust library that builds validators from json scheme rules
https://docs.rs/jsonschema/latest/jsonschema/

Scheduled events, both in creation of ruleset definition and using a runtime function
A scheduled event is a table in the catalog that has a start date and an interval (and maybe an occurrence count) and an action name to run
Maybe also a blob of json to act as a memory? Or the creation function returns an id people can use to refer to the event and therefore remember things about it?
To figure out the next occurrence, we measure the distance since the start date and ceiling divide it by the interval, and multiply that round number by the interval and add it to the start date
The server manages the timers and futures etc to know when to run these. At these times it reads the table and sets up timers to run the next occurrence: at startup, when a new event or ruleset is created
These events need to be smart about checking whether they should even happen. They could be disabled. When we replace a ruleset, we might replace it's recurring events, but only maybe! We can read the segment of the table for the ruleset in question and also iterate over the current timer tasks that fall under that ruleset, and replace them? Or it's probably simpler to just abort all of them beforehand and create new ones after. Since they'll basically certainly be at await points they'll all cancel and then if the exact same thing is created in its place afterward that's fine
For now I'll just run in the actix runtime, and maybe at some point when it seems to matter I'll do something more complicated. Maybe a global async messaging thing would be useful, it receives messages
https://github.com/tokio-rs/tokio/issues/3710
https://docs.rs/tokio-test/0.4.4/tokio_test/





# all the things I could do next:
- figure out how the current rulesets are catalogued
- figure out permissioning and roles (both for the server, and for each ruleset and its different definitions)
- get the action route working (look in the ruleset catalog, invoke the ruleset with its ability to execute queries revoked to gather the function, then invoke that function with its ability to execute queries reinstated with a full writeable role)
- get the view route working (look in the ruleset catalog, invoke the ruleset with its ability to execute queries revoked to gather the function, then invoke that function with its ability to execute queries reinstated with a read-only role)
- get the route working that simply shows views (and actions?) that are available to the user
- figure out the mechanism whereby new constitutions are proposed (from a separate top-level route? probably not, but through a function in a ruleset, which means it's possible to write an unchangable ruleset)
- figure out how the function that proposes new constitutions works, and what it's purpose is (I feel like the purpose is basically to run *whatever* checks for basic well-formedness, such as migration checking, possibly type checking!). it seems a ruleset would have a table that *points to* the global constitution table, and when it wants to switch constitutions it *must*

- figure out how rulesets have children rulesets!

# tests
- sql and http aren't allowed when gathering functions
-


# runtime functions I need to add:
- function to propose new constitution (demands parameter of the constitution, then checks it for well-formedness, then returns an id that can be used to refer to it)
- function to switch a ruleset out for a new constitution (demands parameter of id originally given when it was proposed, returns success or failure)
- get http fetching working, hopefully just import and use implementation from deno
- get sql execution working, mirror the api of sqlx faithfully?


# ruleset examples I could make:
- simple, three yeses with no nos to propose a new constitution and instate it immediately
  - an action that proposes a new constitution (this calls the function provided by the runtime to submit this to the main list)
  - another action that accepts yeses or nos, and either instates a new constitution if it succeeds (and wipes out all other proposals), or removes/archives a proposal if it receives a no, or does nothing if nothing crossed any thresholds
  - a view that looks at all currently proposed constitutions and their vote counts, and has hooks to propose one and yes or no existing ones
  - a view that looks at the current constitution
- need an example with scheduled events, such as a constitutional approval election every whatever period (for testing purposes every 5 minutes or something)


# later stuff
- figure out user management and auth stuff in general
- figure out "membership", partitioning members into rulesets, and the inherently nested nature of that

ruleset passwords are only internal, so they might as well be rotated constantly
