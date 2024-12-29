https://hyper.rs/guides/1/server/echo/
https://github.com/hyperium/hyper/blob/master/examples/service_struct_impl.rs
https://github.com/hyperium/hyper/blob/master/examples/hello-http2.rs
https://github.com/hyperium/hyper/blob/master/examples/params.rs


https://secutils.dev/docs/blog/rust-application-with-js-extensions
https://deno.com/blog/roll-your-own-javascript-runtime



The purpose of this tool is to completely solve the purely technological sub-problems (incidental complexity) of governance, and leave only policy questions (which are sometimes technical!). Those sub-questions are:

- data storage
- server routes/liveness
- event architecture
- role checking

every `governance-instance` is akin to a database. a server running `votebase` can hold multiple `governance-instance`s

every `governance-instance` is essentially a root `policy`, the core primitive. a `policy` inherently allows for itself to have it's internal contents changed by any `process`. a `policy` can be:

- a leaf, meaning it's an `official` (a person who essentially delegated powers to make particular decisions by the policy) (the ability to have *groups* of `official`s is an important one, and these groups need to have the ability to be dynamically sized according to some other calculation or process); or a `document`, meaning it's just a rich text whatever that humans will have to interpret and act according to.
- a branch, meaning it's another `policy`

every policy must specify how it can be changed. should it even be possible to state that a policy *can't* be changed? certainly if that is possible, specifying that is true must be explicit and required

the system must allow for calling arbitrary computations to perform possibly technical work. every external and therefore opaque computation must be declared in the terms of the `votebase` type system, and must have a description of it's purpose (possibly with a minimal length?)

role declarations are also essential

```
// root is special
policy root
  selected by SomeProcess
  // or
  selected by
    // inner things
    // the important thing is that there isn't only *one* way to select/change things
    // the selection criteria are merely a set of data collections, inputs, events, and roles
    // there can be multiple inputs that produce events dynamically that supersede normally declared ones

  // allows for either named references to policies defined elsewhere, or named declarations in place
  official governor
    // is it possible for all official power to be designated in the form of roles?
    role SomeRole
    metadata
      // this is a function that returns the powers and other metadata, such as salary, of this official
      // this should also have a separate call schedule
    selected by
      // in selected by we can declare data tables and inputs
      data
      input
  document mandate
    contents
      Four-score and seven years ago...
    selected by
      ...

  // this policy can be changed individually through it's own selection mechanism, without using the selection mechanism of root
  policy OversightReferendum
  // this policy can't be changed individually, only through root
  ruleset SomethingProcess
```

it's clear there's a difference between a policy's *selection* ruleset and the mere embedded rulesets underneath it

since this needs to be a highly readable language, policy declarations vs "invocations" (statements that actually say the policy is in force rather than merely existing or being imported) need to be possible, perhaps with `using` keyword (`official using governor` vs `official governor`)
`define policy` vs `policy` could be the syntax for this distinction

blaine, focus on semantics, not syntax


the question of policies is quite simple and intuitive, and seems to fit any and all needs of any polity
it seems both `selected by` and `described by` n

it seems we need another concept, that of a `ruleset`, basically a reusable bundle of collections, inputs, events, and roles
this is important *because* of reusability, and not really for any other reason?
the interesting part is how the interaction between a containing policy and a ruleset should work

this reusability has to be composable, so multiple rulesets can be combined in a single policy
it also has to allow for selective overriding (`override input something`)
inputs can be called as normal functions from any other input

the semantics of all these things are clear, they're just data as we underestand them in databases, events as we understand them in things like cron, inputs as we understand them from servers, and roles as we understand them in servers and databases
events are mere primitives, probably better understand as just a sugared type, so I guess the *real* primitive here is "triggers", or `on` events

the interesting thing are the commands to *change* policies
let's say there's a policy that specifies its selection method as some reusable
the only time we're *changing* something that isn't just a data collection is when we're changing a policy

rulesets can take in parameters in the event they need to interact with other unknown rulesets in some way


can all governance be fully implemented by data collections, inputs, events, and roles?
those seem like the minimal primitives
those afterall are the minimal primitives of a database/api combination!

we've said external computations should be allowed

events are interesting, because they ultimately have to be editable and arbitrary
but we want the ability to draw from generators of events

how do we get both?
perhaps this is one of those things we punt?



TODO Elinor Ostrom quote about iteration and local conditions and flexibility

there are two separate things groups need to do to successfully coordinate
- enough members of the group need to realize it's a good idea to coordinate, and have sufficient motivation to do so
- then they need to have a way to communicate, share coordination options, and make binding decisions about those options

- will to coordinate
- ability to communicate
- ability to decide

coordination isn't merely communication, it requires the ability to create precise rules, making binding decisions to commit to those rules, and build other rules to enforce the rules


Examples always win

# Persistent Election

```
ruleset PersistentWeights
  data WeightAllowance
    voter: &VoterType
    weight: numeric & > 0
  data Allocation
    voter: &VoterType
    weight: numeric
  constraints
    sum(abs(Allocation.weight)) <= sum(WeightAllowance.weight)
    group by voter

ruleset PersistentApprovalElection(VoterType, AllocationType, CandidateType)
  // every voter can only have a single allocation in this election at a time, since it's a weighted approval ballot
  // here the consistency of weight sums is left up to higher levels, this ruleset doesn't worry about it
  data Ballot
    voter: &VoterType
    weight_allocation: &AllocationType
    approvals: &CandidateType#{}
    disapprovals: &CandidateType#{}
    // #{} indicates a set, so the candidates have to be unique and non-duplicated
  constraints
    approvals :union disapprovals == #{}

  data Bucket
    candidate: &CandidateType
    total_vote
    accrued
    // TODO look at this next

  event Update = every week on Monday midnight

  // should think about nomination process
  input enter_candidate()
  input withdraw_candidate()

  input update_ballot(weight_allocation: &AllocationType, approvals: &CandidateType[])
    voter = ~caller
    Ballot |find .voter == voter |set { weight_allocation = weight_allocation, approvals = approvals }

  on Update

```

# Persistent Commitment

# Normal Regularly occuring event-based election

```
ruleset EventApprovalElection(VoterType, CandidateType)
  data Candidate; CandidateType
  data Approval
    voter: &VoterType
    candidate: &CandidateType
  constraints; unique (voter, candidate)

  event ElectionOpen = ElectionEnds - 3 months
  event ElectionStarts = ElectionEnds - 1 month
  event ElectionEnds every four years starting 2020, second Thursday of November

  @allowed(from ElectionOpen, upto ElectionStarts)
  input enter_candidate(candidate: CandidateType)
    Candidate |insert { candidate }

  @allowed(owns candidate)
  input withdraw_candidate(candidate: &CandidateType)
    Candidate |find .candidate == candidate |delete

  @allowed(from ElectionStarts, upto ElectionEnds)
  input vote_for_candidate(candidate: &CandidateType)
    voter = ~caller
    Approval |insert { voter, candidate }

  on ElectionEnds
    winner =
      from Approval
      aggregate [group by candidate] votes = sum
      select-by max(votes) candidate

    // handle possible ties

    ~emit-change winner
```

From this code we can infer that CandidateType has to be a policy
and that VoterType has to be a caller? or at least that the caller has to *own* a VoterType

```
ruleset Something
  input yo()
    // a guard-role construct is basically a "priority ordered inclusionary match"
    // the caller
    vote_strength = ~guard-role
      Governor; 5
      Voter; 1
      _; ~reject
```


---

future things to think about

- all data saved needs to be saved in an "event-based" way, meaning the entire history of the entire polity could be reconstructed in precisely auditable detail
- the above needs to be compatible with privacy and secret ballots! zero-knowledge proofs and cryptography etc
