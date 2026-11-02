------------------------------ MODULE MergeGuard ------------------------------
(***************************************************************************)
(* The guard of a merge (principle IV). A run tests its group on a base and *)
(* the PRs of its predecessors; its merge must land on that base followed  *)
(* by the merges of those predecessors, and on nothing else. Two checks    *)
(* keep it: the chain (each predecessor landed on the head before it, as   *)
(* the log records it) and the head (the job reads the real head and      *)
(* merges only on the head that the run expects).                          *)
(*                                                                         *)
(* What the queue believes of the head is any commit of the branch, a past *)
(* one included: the model holds whatever `target_pushed` decides, its     *)
(* bugs included. A group can be dissolved at any time, for any reason.    *)
(* Not modeled: the window between the read of the head and the merge,    *)
(* which the SCM gives no way to close for a squash or a merge commit.     *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS N,        \* the number of PRs of the queue
          Outside,  \* TRUE: one person pushes to the target once
          Fault     \* "none", or the name of a check to drop: see FaultNames

\* no_chain_check: plan does not check that each predecessor landed on the head
\*                 before it.
\* no_head_check:  the merge job merges whatever the real head.
FaultNames == {"none", "no_chain_check", "no_head_check"}

Base == "b0"
X == "x"
None == "none"
NoGroup == [base |-> None, preds |-> <<>>]

\* The PRs of the queue, in its order.
Order == SubSeq(<<"p1", "p2", "p3", "p4">>, 1, N)

ASSUME /\ Fault \in FaultNames
       /\ Outside \in BOOLEAN
       /\ N \in 1..4

\* The code that this model copies (scripts/tla-pins.sh): a change of it
\* fails `mise run lint` until the model is read against it again.
\* pin crates/mq-core/src/snapshot.rs fn expected_head 30acd4580ccc
\* pin crates/mq-core/src/apply.rs region merge-failed-target-moved ccab93e1ee2d
\* pin crates/mergequeue/src/merge.rs fn land d93fe7c677bc

PRs == {Order[i] : i \in 1..Len(Order)}
Ahead(p) == {Order[i] : i \in 1..(CHOOSE j \in 1..Len(Order) : Order[j] = p) - 1}
Range(s) == {s[i] : i \in 1..Len(s)}
Last(s) == s[Len(s)]

VARIABLES main,     \* Seq: the real commits of the target; a PR merges as itself
          belief,   \* the head that the queue believes, any commit of the branch
          group,    \* PR -> NoGroup, or [base, preds]: the run that tested it
          merged,   \* the PRs whose merge.succeeded the log holds
          parent,   \* PR -> the head on which the log records that it landed
          untested  \* TRUE once a merge landed on what its run did not test

vars == <<main, belief, group, merged, parent, untested>>

Pos(c) == CHOOSE i \in 1..Len(main) : main[i] = c

\* The predecessors of a group in the order of the queue, as a sequence.
RECURSIVE InOrder(_, _)
InOrder(set, i) ==
    IF i > Len(Order) THEN <<>>
    ELSE IF Order[i] \in set THEN <<Order[i]>> \o InOrder(set, i + 1)
    ELSE InOrder(set, i + 1)

\* snapshot.rs, expected_head: the base, then the merge of each predecessor,
\* each on the head before it; None when one landed elsewhere.
RECURSIVE Chain(_, _)
Chain(head, preds) ==
    IF preds = <<>> THEN head
    ELSE IF Head(preds) \notin merged THEN head
    ELSE IF Fault # "no_chain_check" /\ parent[Head(preds)] # head THEN None
    ELSE Chain(Head(preds), Tail(preds))

ExpectedHead(p) == Chain(group[p].base, group[p].preds)

Init ==
    /\ main = <<Base>>
    /\ belief = Base
    /\ group = [p \in PRs |-> NoGroup]
    /\ merged = {}
    /\ parent = [p \in PRs |-> None]
    /\ untested = FALSE

\* The queue learns a head, right or wrong.
Learn(c) ==
    /\ c \in Range(main)
    /\ belief' = c
    /\ UNCHANGED <<main, group, merged, parent, untested>>

\* A group forms on the believed head, behind the groups ahead of it.
Form(p) ==
    /\ p \notin merged
    /\ group[p] = NoGroup
    /\ \A q \in Ahead(p) : q \in merged \/ group[q] # NoGroup
    /\ group' = [group EXCEPT ![p] = [base |-> belief, preds |-> InOrder(Ahead(p) \ merged, 1)]]
    /\ UNCHANGED <<main, belief, merged, parent, untested>>

\* Any rule of the queue can dissolve a group; validity does it when the chain
\* is broken.
Dissolve(p) ==
    /\ group[p] # NoGroup
    /\ p \notin merged
    /\ group' = [group EXCEPT ![p] = NoGroup]
    /\ UNCHANGED <<main, belief, merged, parent, untested>>

\* plan asks the merge once every predecessor merged; the job reads the real
\* head and merges only on the expected head, else gives `target_moved`, and
\* the group restarts.
Merge(p) ==
    LET g == group[p]
        expected == ExpectedHead(p)
        tested == g.preds
    IN /\ g # NoGroup
       /\ p \notin merged
       /\ \A q \in Range(g.preds) : q \in merged
       /\ expected # None
       /\ IF Fault = "no_head_check" \/ Last(main) = expected
          THEN /\ main' = Append(main, p)
               /\ merged' = merged \cup {p}
               /\ parent' = [parent EXCEPT ![p] = expected]
               /\ untested' = (untested \/ g.base \notin Range(main)
                               \/ SubSeq(main, Pos(g.base) + 1, Len(main)) # tested)
               /\ UNCHANGED group
          ELSE /\ group' = [group EXCEPT ![p] = NoGroup]
               /\ UNCHANGED <<main, merged, parent, untested>>
       /\ UNCHANGED belief

PushFromOutside ==
    /\ Outside
    /\ X \notin Range(main)
    /\ main' = Append(main, X)
    /\ UNCHANGED <<belief, group, merged, parent, untested>>

\* The end: every PR merged. It stutters, so that TLC reports any other state
\* where nothing can happen as a deadlock.
Done ==
    /\ merged = PRs
    /\ UNCHANGED vars

Next ==
    \/ \E c \in Range(main) : Learn(c)
    \/ \E p \in PRs : Form(p) \/ Dissolve(p) \/ Merge(p)
    \/ PushFromOutside
    \/ Done

Spec == Init /\ [][Next]_vars

(* ----------------------------- invariants ----------------------------- *)

TypeOK ==
    /\ Range(main) \subseteq PRs \cup {Base, X}
    /\ belief \in Range(main)
    /\ merged \subseteq PRs

\* Principle IV: a merge lands on its base followed by the merges of its
\* predecessors, in order, and on nothing else.
MergeLandsOnWhatWasTested == ~untested

(* Witnesses: each must be violated, which proves that the model reaches
   the case. *)

\* Every PR merges, a push from outside included.
NotAllMerged == merged # PRs

\* A merge is refused, then the PR merges after a new run.
NeverMergedAfterOutside == ~(X \in Range(main) /\ \E p \in merged : Pos(p) > Pos(X))
=============================================================================
