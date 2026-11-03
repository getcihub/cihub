---------------------------- MODULE TargetHead ----------------------------
(***************************************************************************)
(* What the queue knows of the head of its target branch, and the bases on *)
(* which its runs stay valid. The merges of the queue, a push from outside *)
(* and the reads of the repair loop change the real branch; the webhooks   *)
(* of those pushes arrive late, in any order, or never; the merge job and  *)
(* the repair job give their facts through `queue_inbox`; a reconciliation *)
(* reads the inbox, then each delivery, at different instants. The rules  *)
(* are those of `apply.rs` (`target_pushed`), `scheduler.rs` (`Pushed`),   *)
(* `repair.rs` (`merge_in_flight`, `read_target`) and `merge.rs`.          *)
(*                                                                         *)
(* The merge job here merges on any head: its head check and the refusal  *)
(* `target_moved` are in `MergeGuard.tla`, which proves that a wrong view  *)
(* of the head costs a run, never an untested merge. This model checks the *)
(* view itself: how often it is wrong, and how it comes back.              *)
(* Not modeled: the groups and the runs (any valid base may hold a group), *)
(* a webhook delivered twice, the limit of `RECENT_HEADS`, the retain of   *)
(* the bases that no group uses.                                           *)
(***************************************************************************)
\* The code that this model copies (scripts/tla-pins.sh): a change of it
\* fails `mise run lint` until the model is read against it again.
\* pin crates/mq-core/src/apply.rs fn target_pushed c558c6dec192
\* pin crates/mq-core/src/apply.rs region merge-succeeded-is-own 9f057466ec8d
\* pin crates/mergequeue/src/scheduler.rs region target-push-waits f97868dd49f3
\* pin crates/mergequeue/src/repair.rs fn merge_in_flight 213da43850cc
\* pin crates/mergequeue/src/repair.rs fn read_target 01942c4a4f55
\* pin crates/mergequeue/src/merge.rs fn land d93fe7c677bc

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS Merges,     \* the merges of the queue, e.g. {"m1", "m2"}
          Outside,    \* TRUE: one person pushes to the target once
          MaxReads,   \* the number of reads of the head by the repair loop
          ReadChecksAgain, \* TRUE: the repair job checks again after its read
          Fault       \* "none", or the name of a rule to break: see FaultNames

\* Each fault is the rule before a fix:
\*   inflight_jobs_only  merge_in_flight counts only the jobs (f05d5e4)
\*   end_before_fact     the merge job ends before it gives its fact
\*   late_push_ignored   a push whose before is a past head is ignored (d95bd82)
\*   own_without_before  a known merge is own whatever its before (d95bd82)
\*   late_own_restarts   a late push of a merge is a late outside push (6d1af32)
FaultNames == {"none", "inflight_jobs_only", "end_before_fact", "late_push_ignored",
               "own_without_before", "late_own_restarts"}

None == "none"
Base == "b0"
X == "x"

ASSUME /\ Fault \in FaultNames
       /\ Outside \in BOOLEAN
       /\ MaxReads \in Nat
       /\ ReadChecksAgain \in BOOLEAN
       /\ {None, Base, X} \cap Merges = {}

VARIABLES main,      \* Seq: the real commits of the target, in order
          phase,     \* merge -> "none", "queued", "merged" or "done": its job
          given,     \* merge -> TRUE once its job gave merge.succeeded
          plannedOn, \* merge -> the head that the queue knew when it asked for it
          sent,      \* the webhooks of the pushes, not delivered yet
          inbox,     \* Seq: the deliveries of webhook_inbox
          facts,     \* Seq: the facts of queue_inbox
          reading,   \* TRUE between the check and the read of the repair job
          reads,     \* the reads of the head done
          k,         \* what the snapshot knows: [head, recent, own, valid]
          seen,      \* each SHA that the queue applied or ignored as an after
          landed,    \* the merges whose merge.succeeded the log holds
          rc,        \* "idle", or "deliveries" during a reconciliation
          todo,      \* Seq: the deliveries that this reconciliation still reads
          spurious,  \* TRUE once a push removed a base that was still right
          untested   \* TRUE once a merge landed on an outside commit

vars == <<main, phase, given, plannedOn, sent, inbox, facts, reading, reads, k,
          seen, landed, rc, todo, spurious, untested>>

Range(s) == {s[i] : i \in 1..Len(s)}
Last(s) == s[Len(s)]
Pos(c) == CHOOSE i \in 1..Len(main) : main[i] = c

\* A base stays right while every commit after it is a merge of the queue.
IsRight(b) == b \in Range(main) /\ \A i \in (Pos(b) + 1)..Len(main) : main[i] \in Merges

\* repair.rs, merge_in_flight: a merge job waits or runs, or its fact waits.
MergeInFlight ==
    \/ \E m \in Merges : phase[m] \in {"queued", "merged"}
    \/ Fault # "inflight_jobs_only" /\ \E i \in 1..Len(facts) : facts[i].kind = "merged"

\* apply.rs, target_pushed: what the snapshot knows after a push or a read.
Pushed(kn, before, after) ==
    LET isPast(s) == s \in kn.recent
        isLate == before # None /\ before # kn.head /\ isPast(before)
        isKnownOwn == after \in kn.own
        own == kn.own \ {after}
        isIgnored == \/ kn.head = after
                     \/ isPast(after)
                     \/ isLate /\ isKnownOwn /\ Fault # "late_own_restarts"
                     \/ isLate /\ Fault = "late_push_ignored"
        isOwn == isKnownOwn /\ (Fault = "own_without_before" \/ before = None \/ before = kn.head)
    IN IF isIgnored THEN [kn EXCEPT !.own = own]
       ELSE IF isLate THEN [kn EXCEPT !.own = own, !.valid = {b \in kn.valid : b = kn.head}]
       ELSE [head |-> after, recent |-> kn.recent \cup {kn.head}, own |-> own,
             valid |-> (IF isOwn THEN kn.valid ELSE {}) \cup {after}]

\* A push removed a base that no outside commit has made wrong.
RemovesRight(kn, kn2) == \E b \in kn.valid \ kn2.valid : IsRight(b)

ApplyFact(kn, f) ==
    IF f.kind = "merged" THEN [kn EXCEPT !.own = kn.own \cup {f.sha}]
    ELSE Pushed(kn, None, f.sha)

\* The facts of queue_inbox, in their order.
RECURSIVE ApplyFacts(_, _)
ApplyFacts(kn, fs) == IF fs = <<>> THEN kn ELSE ApplyFacts(ApplyFact(kn, Head(fs)), Tail(fs))

RECURSIVE SpuriousIn(_, _)
SpuriousIn(kn, fs) ==
    IF fs = <<>> THEN FALSE
    ELSE LET kn2 == ApplyFact(kn, Head(fs))
         IN RemovesRight(kn, kn2) \/ SpuriousIn(kn2, Tail(fs))

Init ==
    /\ main = <<Base>>
    /\ phase = [m \in Merges |-> "none"]
    /\ given = [m \in Merges |-> FALSE]
    /\ plannedOn = [m \in Merges |-> None]
    /\ sent = {}
    /\ inbox = <<>>
    /\ facts = <<>>
    /\ reading = FALSE
    /\ reads = 0
    /\ k = [head |-> Base, recent |-> {}, own |-> {}, valid |-> {Base}]
    /\ seen = {Base}
    /\ landed = {}
    /\ rc = "idle"
    /\ todo = <<>>
    /\ spurious = FALSE
    /\ untested = FALSE

(* ------------------------------ the queue ----------------------------- *)

\* plan asks for one merge at a time, in the order of the graph: once the log
\* holds the merge of the item ahead, of a run on a valid head.
AskMerge(m) ==
    /\ rc = "idle"
    /\ phase[m] = "none"
    /\ \A n \in Merges : phase[n] = "none" \/ n \in landed
    /\ k.head \in k.valid
    /\ phase' = [phase EXCEPT ![m] = "queued"]
    /\ plannedOn' = [plannedOn EXCEPT ![m] = k.head]
    /\ UNCHANGED <<main, given, sent, inbox, facts, reading, reads, k, seen, rc, todo,
                   spurious, untested, landed>>

\* The facts of queue_inbox first, then a snapshot of the deliveries.
BeginReconciliation ==
    /\ rc = "idle"
    /\ k' = ApplyFacts(k, facts)
    /\ seen' = seen \cup {facts[i].sha : i \in {j \in 1..Len(facts) : facts[j].kind = "pushed"}}
    /\ spurious' = (spurious \/ SpuriousIn(k, facts))
    /\ landed' = landed \cup {facts[i].sha : i \in {j \in 1..Len(facts) : facts[j].kind = "merged"}}
    /\ facts' = <<>>
    /\ rc' = "deliveries"
    /\ todo' = inbox
    /\ UNCHANGED <<main, phase, given, plannedOn, sent, inbox, reading, reads, untested>>

\* scheduler.rs: a push that is not known as own waits while a merge is in
\* flight; merge_in_flight reads the tables now, not at the snapshot.
ReadDelivery ==
    LET d == Head(todo)
        waits == d.after \notin k.own /\ MergeInFlight
        k2 == Pushed(k, d.before, d.after)
    IN /\ rc = "deliveries"
       /\ todo # <<>>
       /\ todo' = Tail(todo)
       /\ IF waits
          THEN UNCHANGED <<inbox, k, seen, spurious>>
          ELSE /\ inbox' = SelectSeq(inbox, LAMBDA e : e # d)
               /\ k' = k2
               /\ seen' = seen \cup {d.after}
               /\ spurious' = (spurious \/ RemovesRight(k, k2))
       /\ UNCHANGED <<main, phase, given, plannedOn, sent, facts, reading, reads, rc, untested, landed>>

EndReconciliation ==
    /\ rc = "deliveries"
    /\ todo = <<>>
    /\ rc' = "idle"
    /\ UNCHANGED <<main, phase, given, plannedOn, sent, inbox, facts, reading, reads, k, seen,
                   todo, spurious, untested, landed>>

(* ------------------------------- the jobs ----------------------------- *)

\* merge.rs merges on the real head, whatever the queue knows of it.
Merge(m) ==
    /\ phase[m] = "queued"
    /\ main' = Append(main, m)
    /\ sent' = sent \cup {[before |-> Last(main), after |-> m]}
    /\ phase' = [phase EXCEPT ![m] = "merged"]
    /\ untested' = (untested \/ \E i \in (Pos(plannedOn[m]) + 1)..Len(main) : main[i] \notin Merges)
    /\ UNCHANGED <<given, plannedOn, inbox, facts, reading, reads, k, seen, rc, todo, spurious, landed>>

GiveFact(m) ==
    /\ phase[m] \in {"merged", "done"}
    /\ ~given[m]
    /\ given' = [given EXCEPT ![m] = TRUE]
    /\ facts' = Append(facts, [kind |-> "merged", sha |-> m])
    /\ UNCHANGED <<main, phase, plannedOn, sent, inbox, reading, reads, k, seen, rc, todo,
                   spurious, untested, landed>>

\* The job gives its fact, then ends in another transaction.
EndMerge(m) ==
    /\ phase[m] = "merged"
    /\ given[m] \/ Fault = "end_before_fact"
    /\ phase' = [phase EXCEPT ![m] = "done"]
    /\ UNCHANGED <<main, given, plannedOn, sent, inbox, facts, reading, reads, k, seen, rc, todo,
                   spurious, untested, landed>>

\* repair.rs, read_target: no read while a merge is in flight...
CheckRead ==
    /\ ~reading
    /\ reads < MaxReads
    /\ ~MergeInFlight
    /\ reading' = TRUE
    /\ UNCHANGED <<main, phase, given, plannedOn, sent, inbox, facts, reads, k, seen, rc, todo,
                   spurious, untested, landed>>

\* ...then a read of the real head, given back without a before. A merge can
\* start and land between the check and the read: with ReadChecksAgain, the
\* job checks again after the read and gives nothing while a merge is in flight.
ReadHead ==
    /\ reading
    /\ reading' = FALSE
    /\ reads' = reads + 1
    /\ facts' = IF ReadChecksAgain /\ MergeInFlight THEN facts
                ELSE Append(facts, [kind |-> "pushed", sha |-> Last(main)])
    /\ UNCHANGED <<main, phase, given, plannedOn, sent, inbox, k, seen, rc, todo, spurious,
                   untested, landed>>

(* ----------------------------- the outside ---------------------------- *)

PushFromOutside ==
    /\ Outside
    /\ X \notin Range(main)
    /\ main' = Append(main, X)
    /\ sent' = sent \cup {[before |-> Last(main), after |-> X]}
    /\ UNCHANGED <<phase, given, plannedOn, inbox, facts, reading, reads, k, seen, rc, todo,
                   spurious, untested, landed>>

\* A webhook arrives at any time, in any order, or never.
Deliver(w) ==
    /\ w \in sent
    /\ sent' = sent \ {w}
    /\ inbox' = Append(inbox, w)
    /\ UNCHANGED <<main, phase, given, plannedOn, facts, reading, reads, k, seen, rc, todo,
                   spurious, untested, landed>>

Next ==
    \/ \E m \in Merges : AskMerge(m) \/ Merge(m) \/ GiveFact(m) \/ EndMerge(m)
    \/ BeginReconciliation \/ ReadDelivery \/ EndReconciliation
    \/ CheckRead \/ ReadHead
    \/ PushFromOutside
    \/ \E w \in sent : Deliver(w)

Spec == Init /\ [][Next]_vars

(* ----------------------------- invariants ----------------------------- *)

TypeOK ==
    /\ Range(main) \subseteq Merges \cup {Base, X}
    /\ phase \in [Merges -> {"none", "queued", "merged", "done"}]
    /\ k.valid \subseteq Range(main)
    /\ rc \in {"idle", "deliveries"}

\* Once the queue knows a head at or past the outside commit, no base before
\* it stays valid: a run on that base would land without it (CIH-5).
ViewOfTheHeadIsRight ==
    (X \in Range(main) /\ \E s \in seen : Pos(s) >= Pos(X)) => \A b \in k.valid : IsRight(b)

\* The push of a merge of the queue never restarts the runs on a right base
\* (f05d5e4, 6d1af32): the restart costs a run, and the merge then reads as a
\* merge from outside.
NoSpuriousRestart == ~spurious

(* What a merge job without its head check does: the merge lands on a     *)
(* head that the run did not test. The code checks the head (`MergeGuard`). *)
MergeLandsOnWhatWasTested == ~untested

(* Witnesses: each must be violated, which proves that the model reaches
   the case. *)

\* The queue learns the outside push.
OutsideNeverSeen == X \notin seen

\* Every merge lands.
NotAllMerged == \E m \in Merges : phase[m] # "done"
=============================================================================
