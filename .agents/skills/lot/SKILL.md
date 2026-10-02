---
name: lot
description: State of the current CIHub lot: what is done, what remains, what blocks, the decisions to make. Use when asked "where are we", "what do we have to do now", "let's start lot N", or for a summary before continuing.
---
# State of the lot

Read, without modifying anything:

1. `docs/LOTS.md`: the "Overview" table, then the section of the current lot (or of the requested lot: $ARGUMENTS).
2. The end of `docs/DECISIONS.md`: the latest decisions and the open questions that the lot lists.
3. `git log --oneline -15` and `git status --short`.

Answer in the language of the person, short, in this order:

- **Where we are**: three sentences at most.
- **Done / remaining**: a two-column table, one row per step of the lot.
- **What blocks**: what waits on the project lead (an App, an account, a validation), or "nothing".
- **Decisions to make**: for each one, the context in two sentences, the options, a recommendation. A Mermaid
  diagram when it helps to understand a flow or a state.
- **Proposed next step**: a single one.

Rules: short sentences, no jargon without a definition, no code. If the request is "let's start lot N", add the
proposed steps and the decisions to validate before coding, then wait for the answer.
