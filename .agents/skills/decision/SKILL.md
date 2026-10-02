---
name: decision
description: Record in docs/DECISIONS.md a decision that the project lead has just validated (next Dn number, date, "superseded by" and "extended by" markers, settled open question, documents to update). Use after every "yes, I validate", never before.
---

# Record a decision

Prerequisite: the validation of the project lead is in the conversation. Without it, write nothing and propose first.

1. Find the last identifier: `rg -o '\*\*D[0-9]+\*\*' docs/DECISIONS.md | sort -t D -k 2 -n | tail -1`.
2. Find the block of the day (`**<date>, validated by <name>.**`); create it before "## Open questions" if it is
   missing.
3. Add `- **Dn** …` in the style of the existing entries: what is decided, not why we hesitated. A decision that
   supersedes another says so ("Supersedes Dm"); add to Dm the marker `_Superseded by Dn._` at the end of its
   text. A decision that extends another says so ("Extends Dm"); add to Dm `_Extended by Dn._`. Never change the
   rest of an existing decision.
4. If the decision settles a question `Qk`, write `(Qk)` at the start and remove `Qk` from "Open questions".
5. Update, in the same change: the document of the subject (map in `AGENTS.md`), and the "Tracking" block of the
   lot in `docs/LOTS.md` (validated deviations) if the decision changes the plan of the lot.
6. In the code, cite the decision as `DECISIONS.md Dn` in the comment that carries the constraint.

Decision to record: $ARGUMENTS
