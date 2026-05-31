# Votebase

explanation and summary and philosophy, links to other things
main concepts

guide for hosting your own server (bootstrap cli)
guide for creating a ruleset

guide for interacting with a server?
couple different ways we could go about this:
- the way you were planning, just allowing views to be arbitrary html and therefore apps
- a cli focused way (perhaps at this early stage?)
- a way based on baked in ui concepts? probably not, too rigid




Votebase is a *governable server*. What is a governable server and why would you want one? (TODO maybe constitutional server?)

A server is just a computer program that can send and receive messages, and can save data in some kind of database. Servers are behind the operation of every website on the internet.

Normal servers are controlled by *admins*, some users that actually run the server and have complete control of what it does: what messages it can receive, what messages it sends in response, and what data it holds. A server admin can delete data or change the code of the server in any way they want.

[Many smart people]() have pointed out that since these servers often create *digital social spaces* or community tools, it would be nice if they could be owned and controlled [by the people who use them](). This kind of democratic control could prevent the kind of [platform decay]() that has become common.

But how can groups control a server? Servers are often complicated to update, and you would need to create some legal structure to delegate someone to make the code changes to the server you want made.



This is all *governance* is: the process of many people making rules and decisions about things they are doing together. When a homeowners association decides when its pool should be open and what rules the pool should have, that's governance. When a union of workers decides whether to take a contract they've been offered, that's governance. When a city decides who its elected officials should be, that's governance.

What is governance?
It is basically impossible for nontrivially large groups of people to work together on nontrivially large collective goals without the ability to make rules they all agree to be bound by, and to adjudicate and enforce the rules when someone breaks them
Governance is the process of making and enforcing those rules


Let's go down to the basics


Why a governance server?
When you're making and enforcing rules, you will probably have a bunch of information to track, about people and activities you need to know about in order to adjudicate and enforce the rules
Rules need to be as complex as they need to be! To actually achieve their shared goals you have to track whatever you have to track
We often have institutions with employees to do this, and in our age we can make the foundation of this system a bunch of computer programs that take in information, store it, do calculations on it, and send messages to people when they need to do something

Votebase is a bet that a big meaningful chunk of governance rules can be written as computer code and therefore made fully automated and transparent. The hope is that with the ability to easily create new kinds of governance rules, cooperation between people that was previously too difficult or slow can actually happen, and different communities can experiment with new ways to work together productively and stably

I hope for a Cambrian explosion of experiments and new ways of working together




## The core concepts of Votebase

A Votebase server represents some [**Polity**](https://en.wikipedia.org/wiki/Polity), some group of people with shared goals and the desire to coordinate to achieve those goals.

Every Votebase server has a list of **Members**, people who are allowed to interact with the server.

And for Votebase the most important concept is that of the **Ruleset**, since it is the fundamental unit of governance *code* that can be interacted with and changed.

Members interact with **Rulesets**, which are just chunks of code, possibly with some text included.

A Ruleset can specify a few things that define how it works:

- The **Data Schema** of the Ruleset is what data it can store and where, as well as whatever "built-in functions" the data layer has access to. This layer uses [`postgres sql`](), and is defined in a `schema.sql` file when writing your own Ruleset.
- The available **Actions** of the Ruleset, which are the messages Members can send *to* the Ruleset to change the data stored inside it.
- The available **Views** of the Ruleset, which are the messages Members receive *from* the Ruleset to see what its current state is.
- The **Events** of the Ruleset, which are times when something should happen (specifically when a particular Action should be executed). Events can be dynamically created by Actions (either recurring or one-off), and also *static* Events can be specified in the Ruleset itself (only recurring). A recurring Event is defined by a `start date` and a `recurrence duration` (which for now are limited to an int with a unit, such as days or months or years, and can't do things like "last thursday in november"), a one-off event is defined only by a `date`.
- The **Children** of the Ruleset, which are other Rulesets that can be replaced or destroyed by the parent. Children can be dynamically created by Actions, and *static* Children can be specified in the Ruleset itself.

Actions and Views are both written in [Typescript](), and have access to a special [Votebase runtime](TODO docs page for the runtime) ([built using `deno_core`](https://deno.com/blog/roll-your-own-javascript-runtime) if that's interesting to you).

Since Actions and Views are just Typescript, they can perform arbitrary computations, and can interact with the outside world in any way that's allowed by the Votebase runtime.

Views are only allowed to *read* data from the server database, and are expected to return a string representing [HTML]() or just normal text.

Actions are allowed to *read and write* data in the server database, and can't return any data back.


### The Ruleset Tree

Rulesets form a [tree](https://en.wikipedia.org/wiki/Tree_(abstract_data_type)), meaning Rulesets can create *child* Rulesets. There is always a single "root" Ruleset that holistically forms the basis of the entire server.


Rulesets can have *static* children, meaning children that are specified up front in the Ruleset itself, or *dynamic* children that are created using runtime functions. Dynamic children can be created and destroyed according to the code in the Ruleset, whereas static children always exist.

A parent Ruleset is given the power to delete a child Ruleset, or to replace it. Importantly this power only matters if the Ruleset is written to actually ever *use* this power. If a Ruleset is written in such a way that it will never actually use its ability to delete a child Ruleset, then it is as if it didn't have that power at all.

Why have static children? It allows Rulesets to specify different, possibly easier, rules for changing these children rulesets than for changing the parent Ruleset itself. The reason for the tree is to allow subdivision of decisions, and subsidiarity.

When rulesets are replaced (which is how they are changed), if the new Ruleset doesn't re-specify some particular static child, then that child is deleted. If a still remaining static child or a dynamic child `requires` some type/table/function from that static child, the Candidate will be rejected.

Dynamic children can also be managed during replacement. A list or name regex of dynamic children to keep can be specified, and all others will be deleted. Again, if something is deleted but something is remaining that `requires` it, the Candidate will be rejected.

Whenever a Ruleset is changed, all Candidates to replace it are checked again, and thrown away if they're no longer valid.

When a Ruleset is deleted, all of its children are deleted.



Does this system of parents and children force any governance choices on a Polity? No, since sibling Rulesets can reference each other, which means it's always possible to create governance systems where different Rulesets can all interact as equals. It all simply depends on what any particular "root" Ruleset allows.

For example to create a "federated" system where all of the Rulesets Members actually interact with are all on an equal footing, create a single root Ruleset that allows children to be created in whatever loose way, and then those children can reference each other however they please to form any graph of relationships. The only rules the root Ruleset is responsible for specifying is how this federated arrangement is to be changed. Both structured and unstructured systems are possible.

This graph must always be consistent and no Ruleset can have a `requires` that doesn't exist yet, but it's still possible to create cycles by creating a Ruleset that exposes some things, then another that references that first and exposes some things of its own, and then amend the first to reference the second. Now a cycle exists.


### Ruleset Memberships

This is cool and all, but for now Ruleset membership will be something Rulesets will manage themselves if there's any point in doing so! For now all members will be members of all Rulesets, and any "subsetting" will happen through adhoc permissions tracking that the Rulesets do themselves.


The root Ruleset always has *all* the Members of the server as its Members. However child Rulesets can have their membership specified. When creating either a static or dynamic Ruleset, the "membership criterion" is specified along with it.

A membership criterion is always a "declarative" proposition.

When a Ruleset specifies a membership criterion for a child, only the members of the parent are considered (the membership criteria are joined by `AND`, conjunction)

One of the things a Ruleset can do is specify (in its manifest!) "tags" for members. But these are actually just columns on the member table! (Or computed column functions if they are generated.)

If a Ruleset specifies a new "tag", it has to provide a comprehensive criterion to assign that tag to all existing members, or a default that will actually go into the database or function, possibly using existing tags. New members will have to specify any required

Is it possible to use the definitions of the cols to create a graph of computations to correctly specify the values for even new members? Basically a dag of values that are generated in order to

Or even better, the column itself is actually defined as a generated always! Or rather that you *allow* this, you can't always require it, otherwise this dag of columns would have no starting nodes!

The actual name of the column is basically `"col:{full_path_of_ruleset}:{col_name}"`.

The column is removed or changed according to the fate of the Ruleset. This creates yet another way in which Rulesets can be intertangled.

These membership criteria are added to the server as computed column functions, to avoid performance problems of migrating tables, and to avoid column number limits.
Or maybe *all* of these tags etc should be this way? Mostly to avoid problems with schema migration and
https://www.postgresql.org/docs/current/limits.html#:~:text=onto%204%2C294%2C967%2C295%20pages-,columns%20per%20table,-1%2C600

https://www.graphile.org/postgraphile/computed-columns/

```sql
CREATE FUNCTION person_full_name(person person) RETURNS text AS $$
  SELECT person.given_name || ' ' || person.family_name
$$ LANGUAGE sql STABLE;
```

This design of dynamic member info, together with Ruleset member subsetting through criteria statements, allows the trees to subdivide

It also means we have a principled way of requiring and specifying what values members must provide when they join! If a member value doesn't have a default then they have to specify it.

These non-computed values must actually be stored as columns in the database.

We can actually stop *parent* Rulesets from referencing their childrens' anything, member values, exports, etc. Only sibling->sibling or child->parent.




---

# Guides

## Creating your own Ruleset

Creating your own Ruleset will usually pass through all the commands of the `votebase` cli, so I'll explain them in order.

### `init`

`init` sets up a directory with the skeleton of a new Ruleset:

```bash
votebase init some_ruleset_directory
```

which creates the files specified below. Some of these are optional, meaning you can delete them if you don't need them.

- `ruleset.ts`: specifies the actual code of the Ruleset, which should call `votebase.Action` and `votebase.View` to register the Actions and Views. Empty examples of both kinds are given.
- `schema.sql` **optional**: specifies the data schema of the Ruleset, using postgres sql. If you don't need a schema just delete this file, and it will be assumed your Ruleset will store no data of its own.
- `queries` **optional**: a directory where you can place sql files that will be made available as Typescript functions for your Ruleset to use. You can write a single query, a single statement, or even multiple statements separated by `;`. For single queries and statements you can specify [query parameters]() for these queries that will be filled in by using the form `:parameter_name` instead of `$1` etc, and these parameters will be arguments to the Typescript function.
- `ruleset.json` **optional**: information about the ruleset that isn't captured by the above files, which might not be necessary for your Ruleset. Must have the format implied by this Rust struct:

```rust
/// The format for the `ruleset.json` file that describes a reusable Ruleset.
struct RulesetJson {
  /// A mapping of "var" names to objects that must be available to this Ruleset, which can be tables or types or functions.
  requires: HashMap<String, RequireDescription>,
  // member_attributes: HashMap<String, MemberAttribute>,
  // - `member attributes`? this is the place where the declarative statements for new member attributes that are being added go?
}

enum RequireDescription {
  // https://www.postgresql.org/docs/current/ddl-priv.html#DDL-PRIV-SELECT
  // https://www.postgresql.org/docs/current/ddl-priv.html#DDL-PRIV-REFERENCES
  Table { queryable: bool, referenceable: bool, structure: TableDescription },
  // https://www.postgresql.org/docs/current/ddl-priv.html#DDL-PRIV-USAGE
  Type { structure: TypeDescription },
  // https://www.postgresql.org/docs/current/ddl-priv.html#DDL-PRIV-EXECUTE
  QueryFunction { structure: FunctionDescription },
  ActionFunction { structure: FunctionDescription },
}

struct TableDescription {
  // TODO perhaps use the ColumnInfo struct you already have instead?
  columns: Vec<PgType>,
  /// A list of lists of columns, each list representing a unique constraint across some number of columns.
  unique_constraints: Vec<Vec<String>>,
  // do we need anything else? should columns
}

struct PgType {
  typ: Typ,
  nullable: bool,
}

// enum MemberAttribute {
//   Stored { typ: PgType },
//   Computed { computation_sql: String },
// }
```

### `dev`

The `dev` command prepares all the `queries` into `queries.ts`, erroring if something is wrong. This stays at the "abstract" level, so it doesn't need a concrete server to compare against.

This implies a certain amount of checking that the schema of your Ruleset makes sense.

```bash
votebase dev
```

- in a temp podman postgres
  - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
  - generate the queries and write them into the file

### `check`

`check` is similarly abstract, so it type checks the Ruleset, keeping all abstract table/function names abstract. More for checking internal consistency, mostly to ensure a templated Ruleset is correct. This also checks the schema of your Ruleset makes sense.

```bash
votebase check
```

- in a temp podman postgres
  - execute `schema.sql` against it, including rendering placeholders for any abstract requires by choosing random "real" names for each
  - generate the queries and write them into the file
- using a temp podman typescript, runs a typecheck with the votebase tsconfig


Both `dev` and `check` allow a `--vars` option that makes the operation act against a *concrete* Ruleset. This is for when you want to make a concrete one from the beginning.

### `generate_migration`

Create a first draft to go from the schema specified in the current server information, to the one you've specified. You might want to modify the migration.

This overwrites any existing migration!

### `bundle`

`bundle` type checks a Ruleset against a real intended server schema, using a `vars.json` file to specify any templated values if necessary, then places everything into a `ruleset.bundle.json` file ready to be given as a Candidate for that server.

A Votebase server is a holistic entity, and Rulesets as a concept only exist to allow flexibility in dividing up pieces of decision making to be done in different ways. The holistic nature however means that preparing a new candidate Ruleset has to take the entire server into account. This holism is necessary in order to allow many very natural and practical governance patterns, such as interactions between institutions or across levels of governance.

The `vars.json` file should match the format implied by this Rust struct:

```rust
struct RulesetVars {
  /// an optional mapping from the abstract "var" name in this Ruleset to the fully qualified name actually intended
  db_uses: HashMap<String, String>,

  // /// a list of fully qualified Views that this Ruleset relies on
  // view_uses: Vec<String>
  /// /// a list of fully qualified Actions that this Ruleset relies on
  // action_uses: Vec<String>

  /// a mapping from names to KeepOrReplace of pairings of further ruleset directories and vars
  static_children: HashMap<String, KeepOrReplace<(String, RulesetVars)>>,
  /// a predicate that determines what dynamic children to keep, and all others will be recursively deleted
  dynamic_children_keep_rule: String,

  /// a list of static recurring actions of this Ruleset. all others not mentioned here are deleted
  static_recurring_events: Vec<StaticRecurringEvent>,
  /// a predicate that determines what dynamic recurring children to keep, and all others will be recursively deleted
  dynamic_recurring_event_keep_rule: String,
  /// a predicate that determines what dynamic standalone children to keep, and all others will be recursively deleted
  dynamic_standalone_event_keep_rule: String,
}
```


<!-- https://www.postgresql.org/docs/current/app-pgrestore.html -->

- fetch the pg archive and the full Ruleset tree from the real server with whatever caching rules (???) (hash the schema, and when the dev tools request the schema they can specify which one they already have including null, and server tells them they're good if nothing's changed)
- in a temp podman postgres
  - restore the pg archive to a "current" db
  - fulfill `schema.sql` using the actual provided values in `vars`, then write it to an "intended" db, then get a diff from "current" to "intended", using the `schema` parameter to narrow to only this ruleset. then apply that diff as the migration on top of the archive, to be used for real typechecking. can do shenanigans with diffing the archive against nothing with a schema narrowing to get the "current" standalone schema, perhaps
  - generate the queries and write them into the file
- using a temp podman typescript, run a typecheck with the votebase tsconfig
- analyze the ruleset by executing it, and use the migration generated above to create the bundle

When the server gives out this "preparatory" schema, perhaps it only gives the exported objects? That way it's much smaller and more amenable to inspection.

```rust
/// this definition is used both for candidates and entirely new rulesets
/// for entirely new rulesets, to specify "keep" for a static child makes no sense and will be rejected
/// similarly to give anything but the nonempty keep rules for any of the dynamic things will be rejected
struct PackagedRuleset {
  // /// the name? is this necessary? given that ruleset candidates are given against *named slots* I think this is just not needed
  // name: String,
  /// the typescript code representing all the Actions and Views
  code: String,
  /// the final intended schema, used to check that the db_migration is correct
  db_schema: String,
  /// the migration intended to actually be run to reach the state of db_schema
  db_migration: String,
  /// the fully qualified names of all the database objects this Ruleset uses as its requires. derived from looking at the params in the vars and cross-referencing
  db_uses: Vec<String>,
  /// a list of fully qualified Views that this Ruleset relies on. not bothering with this since javascript is easy to share with modules or whatever, but database objects relate to *state*, so that's more necessary
  // view_uses: Vec<String>
  /// a list of fully qualified Actions that this Ruleset relies on
  // action_uses: Vec<String>
  /// a mapping of the static children of this Ruleset. any ruleset not mentioned here is deleted
  static_children: HashMap<String, KeepOrReplace<PackagedRuleset>>,
  /// a predicate that determines what dynamic children to keep, and all others will be recursively deleted
  dynamic_children_keep_rule: String, // maybe dynamic children is the scope to cut?

  /// a list of static recurring actions of this Ruleset. all others not mentioned here are deleted
  static_recurring_events: Vec<StaticRecurringEvent>,
  /// a predicate that determines what dynamic children to keep, and all others will be recursively deleted
  dynamic_recurring_event_keep_rule: String,
  /// a predicate that determines what dynamic children to keep, and all others will be recursively deleted
  dynamic_standalone_event_keep_rule: String,
}

enum KeepOrReplace<R> {
  Keep,
  Replace(R),
}
```

When running the `bundle` command, your specifications will imply changes and deletions to rulesets, and the cli will show these to you to make sure you're okay with them. when bundling an entirely new Ruleset, this won't really happen since it isn't that relevant. when bundling a replacement one it will show you which specific Rulesets will be kept, replaced (and with what), and deleted, and in a replacement will show what children and events will be kept or deleted.






Rulesets can be arbitrary code that can accept any input, but the idea of a Packaged Ruleset Candidate is a reusable concept that Ruleset Actions can accept in a standard way. The `votebase` cli can be used to bundle a Ruleset so it can be provided to an Action.

You need to provide the directory the Ruleset code is located in (can be the current directory `.`), and a `vars.json` file which at least includes a `name` for the final bundled Ruleset.

```bash
# writes to my_ruleset_directory.json directly next to my_ruleset_directory
votebase bundle --server https://my.server --ruleset my_ruleset_directory --vars my_vars.json
```

This `ruleset.json` file can now be provided to the [file upload ruleset input component]() provided in the votebase standard library.

You can call bundle with different output options.

```bash
# accepts a flag --output that writes the ruleset to a different filename
votebase bundle --server https://my.server --ruleset my_ruleset_directory --vars my_vars.json --output my_ruleset_1.json
# accepts a flag --output-stdout that outputs the Packaged Ruleset to stdout
votebase bundle --server https://my.server --ruleset my_ruleset_directory --vars my_vars.json --output-stdout
```




## Starting and maintaining a Votebase server

The votebase server itself is just a [docker image](TODO location of pullable image), so any place you can get a docker image running and visible to the internet, you're good to go. It needs access to a Postgres database.

First you need to set up a postgres database, which you can do any way you like. Once you've done so, follow these steps:

<!-- TODO blaine it would be cool to make the bootstrap cli ask for all these things if they aren't provided, so power users can provide a connection url but otherwise it asks for them -->

```bash
# TODO correct?
cargo install votebase_admin_cli

# these two should be the overall admin username/password for your postgres database. This username/password will *not* be the one used by the votebase server! This is the only place you should ever need to use your overall admin username/password!
YOUR_ADMIN_DB_USERNAME=<fill in here>
YOUR_ADMIN_DB_PASSWORD=<fill in here>

DB_HOST=<fill in here> # determined by your postgres database hosting situation
DB_PORT=<fill in here> # determined by your postgres database hosting situation
DB_NAME=<fill in here> # chosen by you! you must have already created this database

VOTEBASE_SERVER_PASSWORD=$(votebase_admin_cli bootstrap "postgres://$YOUR_ADMIN_DB_USERNAME:$YOUR_ADMIN_DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME")
echo $VOTEBASE_SERVER_PASSWORD
```

The `VOTEBASE_SERVER_PASSWORD` above can now be provided to the docker image of Votebase itself.

Here are all the environment variables you need to provide to the server. `DB_HOST`, `DB_PORT`, `DB_DATABASE` should all be the same as those above.

- `DB_HOST` and `DB_PORT`: host and port your postgres server is running on
- `DB_NAME`: the database name the server should use, which needs to have already been created
- `VOTEBASE_SERVER_PASSWORD`: the password the server itself should use to access the database, representing its admin privileges
- `VOTEBASE_HOST` and `VOTEBASE_PORT`: the host and port the server itself should listen on, (TODO blaine this should probably just always be 0.0.0.0 and 80/443 or whatever right?)
- `DATABASE_MAX_CONNECTIONS`: (optional) the max number of database connections the server should add to its pool, defaults to 5

Above when you called `votebase_admin_cli bootstrap`, a few things happened:

- `DB_NAME` was loaded with the overall votebase database schema.
- A user `votebase_server` was created to act as the user for the votebase server itself.
- A random password was generated and assigned to the above `votebase_server`.
- **The blank "accept any" Ruleset was loaded into the database.** This ["accept any"](TODO location of accept-any definition) is a minimal Ruleset that literally does nothing other than accept the first Ruleset proposed to the `__insert_initial` Action.



TODO the accept any thing makes sense for an *internal* tool for `votebase.org` that defers a real Ruleset, but honestly it probably makes way more sense to just provide the first initial Ruleset immediately? Even for an internal tool, the web interface should accept an initial and pipe it into the database.

<!--
## (Someday) Using votebase.org to create a new Polity

votebase.org/polity/new

click the button
give payment info, and even a means for splitting costs across members
give your polity a name
provide an initial Ruleset to be loaded, even an abstract one and then the vars are prompted if they exist

## (Someday) Once votebase is a language or a set of macros over a database language

Instead of all these json declarations of requires, it will be basically the coq/lean concept of parameterized modules, where the parameters can be types or functions etc.

Then it's all just in the type system baby.
 -->




---

## Runtime Reference

Mostly you should go look at [runtime.ts]

The `votebase.Action` and `votebase.View` functions are used in `ruleset.ts` to define Actions and Views. These functions take a `name` that defines the name of the Action or View, and a Typescript function that defines their actual code. Here are the full signatures:

```ts
TODO

function proposeSelfReplacement(candidate: CandidateRuleset): Promise<string>

// TODO needs to account for possibility of failure
function proposeSelfReplacement(candidate: CandidateRuleset): Promise<string>

function createChildRuleset(name: string, initial: ConcreteRuleset): Promise<string>
function deleteChildRuleset(name: string): Promise<void>
function proposeChildReplacement(name: string, candidate: CandidateRuleset): Promise<string>
function replaceChild(uuid: string): Promise<void>





// function RecurringAction(definition: RecurringAction): void

function createRecurringAction<Arg extends JsonValue>(definition: RecurringAction<Arg>): Promise<string>
function removeRecurringAction(uuid: string): Promise<void>
function scheduleAction<Arg extends JsonValue>(description: string, at: Date, action: FnAction<Arg>, arg: Arg): Promise<string>
function unscheduleAction(uuid: string): Promise<void>

function enrollMember(email: string): Promise<string>
function removeMemberByEmail(email: string): Promise<void>
function removeMemberByUuid(uuid: string): Promise<void>

function addMembersToRuleset(full_path: string, uuids: string[]): Promise<void>
// function addMembersToRulesetByCondition(full_path: string, condition: string): Promise<void>
function removeMembersFromRuleset(full_path: string, uuids: string[]): Promise<void>
```
