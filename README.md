A governance-focused language needs to include persistence/state management and event-based actions as primitives.

Examples always win

# Persistent Election

# Persistent Commitment

# Persistent Budget

# Normal Regularly occuring event-based election

```
type Person = record;
  name: Text
also data; store list; Person

data Governor;
  store &Person

type GovernorCandidate = record;
  candidate: &Person
also data;
  store list; GovernorCandidate
  constraints; unique candidate

event GovernorRaceOpen = every first Monday of June, starting from 2020 every four years
event GovernorElectionStarts = every first Thursday of November, starting from 2020 every four years
event GovernorElectionEnds = every third Thursday of November, starting from 2020 every four years

#only_open(from GovernorRaceOpen, to including GovernorElectionEnds)
input enter_governor_candidate(candidate: &Person);
  GovernorCandidate :append { candidate }

type GovernorVote = record;
  voter: &Person
  candidate: &GovernorCandidate
also data;
  store list; GovernorVote
  constraints; unique voter

#only_open(from GovernorElectionStarts, to including GovernorElectionEnds)
input vote_for_governor(candidate: &GovernorCandidate);
  voter = get_caller
  GovernorVote :append { voter, candidate }

on GovernorElectionEnds :next day;
  winner =
    from GovernorVote
    aggregate [group by candidate] votes = sum
    sort by votes, largest first
    take 1
    select candidate

  Governor = winner

  clear GovernorCandidate
  clear GovernorVote
```
