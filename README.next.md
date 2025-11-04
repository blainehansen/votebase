# Votebase

Votebase is a *governable server*. What is a governable server and why would you want one?

A server is just a computer program that can send and receive messages, and can save data in some kind of database. Servers are behind the operation of every website on the internet.

Normal servers are controlled by *admins*, some users that actually run the server and have complete control of what it does: what messages it can receive, what messages it sends in response, and what data it holds. A server admin can delete data or change the code of the server in any way they want.

[Many smart people]() have pointed out that since these servers often create *digital social spaces* or community tools, it would be nice if they could be controlled [by the people who use them](). This kind of democratic control could prevent the kind of [platform decay]() that has become common.

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

A Votebase server represents some [**Polity**](https://en.wikipedia.org/wiki/Polity), some group of people with some shared goals and the desire to coordinate to achieve those goals.

Every Votebase server has a list of **Members**, people who are allowed to interact with the server.

These Members interact with **Rulesets**, which are just chunks of code, possibly with some text included.

A Ruleset can specify a few things that define how it works:

- The **Data Schema** of the Ruleset is what data it can store and where, as well as whatever "built-in functions" the data layer has access to. This layer uses [`postgres sql`](), and is defined in a `schema.sql` file when writing your own Ruleset.
- The available **Actions** of the Ruleset, which are the messages Members can send *to* the Ruleset to change the data stored inside it.
- The available **Views** of the Ruleset, which are the messages Members receive *from* the Ruleset to see what its current state is.
- The **Events** of the Ruleset, which are times when something should happen (specifically when a particular Action should be called). Events can be dynamically created by Actions (either recurring or one-off), and also *static* Events can be specified in the Ruleset itself (only recurring). A recurring Event is defined by a `start date` and a `recurrence duration` (which for now are limited to an int with a unit, such as days or months or years, and can't do things like "last thursday in november"), a one-off event is defined only by a `date`.
- The **Children** of the Ruleset, which are other Rulesets that can be replaced or destroyed by the parent. Children can be dynamically created by Actions, and *static* Children can be specified in the Ruleset itself.

Actions and Views are both written in [Typescript](), and have access to a special Votebase runtime [built using `deno_core`](https://deno.com/blog/roll-your-own-javascript-runtime). They are defined in the `ruleset.ts` file when writing your own Ruleset.

Since Actions and Views are just Typescript, they can perform arbitrary computations, and can interact with the outside world in any way that's allowed by the Votebase runtime.

Views are only allowed to *read* data from the server database, and are expected to return a string representing [HTML]() or just normal text.

Actions are allowed to *read and write* data in the server database, and can't return any data back.

The `votebase.Action` and `votebase.View` functions are used in `ruleset.ts` to define Actions and Views. These functions take a `name` that defines the name of the Action or View, and a Typescript function that defines their actual code. Here are the full signatures:

```ts
TODO
```

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






### The Ruleset Tree

Rulesets form a [tree](https://en.wikipedia.org/wiki/Tree_(abstract_data_type)), meaning Rulesets can create *child* Rulesets. There is always a single "root" Ruleset that holistically forms the basis of the entire server.


Rulesets can have *static* children, meaning children that are specified up front in the Ruleset itself, or *dynamic* children that are created using runtime functions. Dynamic children can be created and destroyed according to the code in the Ruleset, whereas static children always exist.

A parent Ruleset is given the power to delete a child Ruleset, or to replace it. Importantly this power is only actually *exercised* if the Ruleset is written to actually ever *use* this power. If a Ruleset is written in such a way that it will never actually *use* its ability to delete a child Ruleset, then it is as if it didn't have that power at all.

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





## Designing and submitting a Ruleset

You can use the `votebase` [command-line tool]() to create and package your Ruleset.

`dev` command prepares all the `queries` into `queries.ts`, erroring if something is wrong.
`check` type checks the Ruleset, keeping all abstract table/function names abstract. More for checking internal consistency, mostly to ensure a templated Ruleset is correct.
`compile` type checks the Ruleset against the real intended server schema, using the `vars.json` file specifying any templated values if necessary, then places everything into a `_compiled.json` file ready to be given as a Candidate.


When type checking an abstract Ruleset:

- required tables/functions are given random "real" names and merely declared to exist (created in proper order if the tables rely on each other), templated into the actual sql, and then put into a local postgres db starting using a podman internal process
- the sql checking and query preparation process occurs
- typescript type checking occurs, using a podman internal process

When type checking a concrete Ruleset:

- the real server schema this will drop into is put into the local db, importantly only the "exposed" version of it that is simplified and so much smaller
- the values in `vars.json` are templated into the actual sql, with errors happening if things don't make sense at this point with the way `vars.json` aligns with the real previously input server schema. also query preparation occurs.
- typescript type checking occurs.

When the server gives out this "preparatory" schema, perhaps it only gives the exported objects? That way it's much smaller and more amenable to inspection.






### Design a new Ruleset from scratch

Rulesets have these files:

- `ruleset.ts`: specifies the actual code of the Ruleset, which should call `votebase.Action` and `votebase.View` to register the Actions and Views.
- `schema.sql` **optional**: specifies the data schema of the Ruleset, using postgres sql. If you don't have this file, it will be assumed your Ruleset will store no data of its own, and so the schema will simply be blank.
- `queries` **optional**: a directory where you can place sql files that will be made available as typesafe functions for your Ruleset to use. You can write a single query, a single statement, or even multiple statements separated by `;`. You can specify [query parameters]
- `ruleset.json`: specifies:
  - `exports: string[]` **optional**: A list of table and function names that can be queried/referenced and called (respectively) by external Rulesets. (Do this with sql comments or something instead?)
  - `requires: { [type/table/function name]: signature }` **optional**: A mapping of abstract table/function names and the signature expected. All the table/function names can be referenced as `$table/function name` in your `schema.sql` and `queries` files. These values are then fulfilled by a `vars.json` file when using `compile`. The real tables/functions in `vars.json` don't have to match perfectly, but merely must be "assignable" from a type perspective. (Blaine it's likely this would need to be separated into the different use cases for permissions reasons, as in `references`, `reads`, `calls_readonly`, `calls_mut` or something. this also allows us to check more granularly that these things are even possible given the finally vars object)
  - `member attributes`? this is the place where the declarative statements for new member attributes that are being added go?

### Use a template Ruleset

- `vars.json`: specifies the values for templated values in the ruleset. This can be used to specify:
  - `name: string`: The name of the Ruleset, which must be a valid Ruleset name consisting of only lower case letters and the `_` character (?). This is put in `vars.json` because the name must be unique in context in the real Votebase server.
  - `params: { [type/table/function name]: real name }`: specifications of the types/tables/functions in the real server that fill in for the `requires`.


## Starting and maintaining a Votebase server
