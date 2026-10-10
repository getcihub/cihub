---
name: lot
description: State of the current CIHub lot: what is done, what remains, what blocks, the choices to make. Use when asked "where are we", "what do we have to do now", "let's start lot N", or for a summary before continuing.
---
# State of the lot

Read, without modifying anything:

1. `specs/LOTS.md`: the "Overview" table, then the section of the current lot (or of the requested lot: $ARGUMENTS),
   with its open questions.
2. `git log --oneline -15` and `git status --short`.

Answer in the language of the person, short, in this order:

- **Where we are**: three sentences at most.
- **Done / remaining**: a two-column table, one row per step of the lot.
- **What blocks**: what waits on the project lead (an App, an account, a validation), or "nothing".
- **Choices to make**: only those that are costly to reverse (`AGENTS.md`). For each one, the context in two
  sentences, the options, a recommendation. A Mermaid diagram when it helps to understand a flow or a state.
- **Proposed next step**: a single one.

Rules: short sentences, no jargon without a definition, no code. If the request is "let's start lot N", add the
proposed steps and the choices to validate before coding, then wait for the answer.
