------------------------------- MODULE Queue -------------------------------
(***************************************************************************)
(* One queue in simple mode: the order of its items, a speculative run on  *)
(* top of the items ahead, the merge job, the answer of the SCM, and the   *)
(* insertion of `urgent`. It checks principle IV: what lands on the target *)
(* branch is exactly what a run tested.                                    *)
(*                                                                         *)
(* The facts (the CI reports a run, the SCM answers a merge) can happen at *)
(* any time; the decisions of `plan` happen only when their guard holds.   *)
(* Not modeled: batches, bisection, the failure budget, the speculation    *)
(* window, direct merge, `not_ready` and `cancelled`, outside pushes, and  *)
(* the inbox between a fact and its decision.                              *)
(***************************************************************************)
\* The code that this model copies (scripts/tla-pins.sh): a change of it
\* fails `mise run lint` until the model is read against it again.
\* pin crates/mq-core/src/graph.rs fn insertion_index f0f7c79d62c5
\* pin crates/mq-core/src/apply.rs region merge-succeeded-is-own 1058d07a58e0

EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS PRs,    \* the PRs of the universe, model values
          Fault   \* "none", or the name of a rule to break: see FaultNames

\* urgent_first: `urgent` enters in front of every item that is not urgent,
\* `tests_passed` included, the rule before commit 4106481.
FaultNames == {"none", "urgent_first"}
ASSUME Fault \in FaultNames

VARIABLES order,     \* Seq of the live items, by position; the head merges first
          state,     \* the ItemState of each submitted PR
          urgent,    \* the PRs submitted with the priority `urgent`
          runOn,     \* PR -> what its last run contains under it
          inFlight,  \* PR -> what the run behind its merge job contained
          main       \* Seq of [pr, tested]: the merges on the target, in order

vars == <<order, state, urgent, runOn, inFlight, main>>

ItemStates == {"pending", "testing", "pending_failure", "tests_passed",
               "merged", "failed"}

\* The states whose run a change of the predecessors makes invalid.
WithRun == {"testing", "pending_failure", "tests_passed"}

Range(s) == {s[i] : i \in 1..Len(s)}
Min(set) == CHOOSE i \in set : \A j \in set : i <= j
Index(p) == CHOOSE i \in 1..Len(order) : order[i] = p
Ahead(p) == {order[j] : j \in 1..(Index(p) - 1)}
Behind(p) == {order[j] : j \in (Index(p) + 1)..Len(order)}
Merged == {main[i].pr : i \in 1..Len(main)}
Remove(s, p) == SelectSeq(s, LAMBDA x : x # p)
InsertAt(s, k, p) == SubSeq(s, 1, k - 1) \o <<p>> \o SubSeq(s, k, Len(s))

\* The runs of `set` no longer test what would land: their items go back to
\* `pending`. A merge job already sent is not taken back.
Restart(set) ==
    [q \in DOMAIN state |->
        IF q \in set /\ state[q] \in WithRun THEN "pending" ELSE state[q]]

Init ==
    /\ order = <<>>
    /\ state = [p \in {} |-> "pending"]
    /\ urgent = {}
    /\ runOn = [p \in PRs |-> {}]
    /\ inFlight = [p \in {} |-> {}]
    /\ main = <<>>

(* ------------------------------ requests ------------------------------ *)

\* Any priority other than `urgent` is `medium` here: it enters at the end.
Submit(p) ==
    /\ p \notin DOMAIN state
    /\ order' = Append(order, p)
    /\ state' = state @@ (p :> "pending")
    /\ UNCHANGED <<urgent, runOn, inFlight, main>>

\* graph::insertion_index: in front of the first item that is neither
\* `urgent` nor `tests_passed`, since a tested item may already be merging.
EntersBefore(q) ==
    /\ q \notin urgent
    /\ Fault = "urgent_first" \/ state[q] # "tests_passed"

SubmitUrgent(p) ==
    LET candidates == {i \in 1..Len(order) : EntersBefore(order[i])}
        k == IF candidates = {} THEN Len(order) + 1 ELSE Min(candidates)
    IN /\ p \notin DOMAIN state
       /\ order' = InsertAt(order, k, p)
       /\ state' = Restart({order[j] : j \in k..Len(order)}) @@ (p :> "pending")
       /\ urgent' = urgent \cup {p}
       /\ UNCHANGED <<runOn, inFlight, main>>

(* ----------------------------- decisions ------------------------------ *)

\* A run on the target branch and on every item ahead, while they are all
\* still on their way to merge.
StartRun(p) ==
    /\ p \in DOMAIN state /\ state[p] = "pending"
    /\ \A q \in Ahead(p) : state[q] \in {"testing", "tests_passed"}
    /\ state' = [state EXCEPT ![p] = "testing"]
    /\ runOn' = [runOn EXCEPT ![p] = Merged \cup Ahead(p)]
    /\ UNCHANGED <<order, urgent, inFlight, main>>

StartMerge(p) ==
    /\ order # <<>> /\ Head(order) = p
    /\ state[p] = "tests_passed"
    /\ p \notin DOMAIN inFlight
    /\ inFlight' = inFlight @@ (p :> runOn[p])
    /\ UNCHANGED <<order, state, urgent, runOn, main>>

\* All its predecessors merged: the failed item is the culprit.
DeclareFailed(p) ==
    /\ order # <<>> /\ Head(order) = p
    /\ state[p] = "pending_failure"
    /\ order' = Remove(order, p)
    /\ state' = [Restart(Behind(p)) EXCEPT ![p] = "failed"]
    /\ UNCHANGED <<urgent, runOn, inFlight, main>>

(* ------------------------------- facts -------------------------------- *)

RunSucceeded(p) ==
    /\ p \in DOMAIN state /\ state[p] = "testing"
    /\ state' = [state EXCEPT ![p] = "tests_passed"]
    /\ UNCHANGED <<order, urgent, runOn, inFlight, main>>

RunFailed(p) ==
    /\ p \in DOMAIN state /\ state[p] = "testing"
    /\ state' = [state EXCEPT ![p] = "pending_failure"]
    /\ UNCHANGED <<order, urgent, runOn, inFlight, main>>

\* apply.rs, MergeSucceeded: the push of a merge is the queue's own only when
\* the item is still `tests_passed`; any other push makes every run invalid.
MergeSucceeded(p) ==
    LET isOwn == state[p] = "tests_passed"
        others == Range(order) \ {p}
        after == IF isOwn THEN state ELSE Restart(others)
    IN /\ p \in DOMAIN inFlight
       /\ main' = Append(main, [pr |-> p, tested |-> inFlight[p]])
       /\ inFlight' = [q \in DOMAIN inFlight \ {p} |-> inFlight[q]]
       /\ order' = Remove(order, p)
       /\ state' = [after EXCEPT ![p] = "merged"]
       /\ UNCHANGED <<urgent, runOn>>

(* -------------------------------- spec -------------------------------- *)

\* The end: every PR merged or failed. It stutters, so that TLC still
\* reports any other state where nothing can happen as a deadlock.
Done ==
    /\ DOMAIN state = PRs
    /\ \A p \in PRs : state[p] \in {"merged", "failed"}
    /\ DOMAIN inFlight = {}
    /\ UNCHANGED vars

Next ==
    \/ \E p \in PRs :
        \/ Submit(p) \/ SubmitUrgent(p)
        \/ StartRun(p) \/ StartMerge(p) \/ DeclareFailed(p)
        \/ RunSucceeded(p) \/ RunFailed(p) \/ MergeSucceeded(p)
    \/ Done

Spec == Init /\ [][Next]_vars

Symmetry == Permutations(PRs)

(* ----------------------------- invariants ----------------------------- *)

TypeOK ==
    /\ DOMAIN state \subseteq PRs
    /\ \A p \in DOMAIN state : state[p] \in ItemStates
    /\ Range(order) \subseteq DOMAIN state
    /\ urgent \subseteq DOMAIN state
    /\ runOn \in [PRs -> SUBSET PRs]
    /\ DOMAIN inFlight \subseteq PRs
    /\ \A i \in 1..Len(main) : main[i].pr \in PRs /\ main[i].tested \subseteq PRs

\* Principle IV: each merge was tested on exactly the merges under it.
TestedIsWhatLands ==
    \A i \in 1..Len(main) : main[i].tested = {main[j].pr : j \in 1..(i - 1)}

(* Witnesses: each must be violated, which proves that the model reaches
   the case. A witness that holds means that a check above is empty. *)

\* An urgent item arrives while a merge job is in flight.
NoUrgentDuringMerge ==
    ~(\E p \in DOMAIN inFlight : \E u \in urgent : u \in Range(order) /\ u # p)

\* Every PR merges.
NotAllMerged == Len(main) < Cardinality(PRs)
=============================================================================
