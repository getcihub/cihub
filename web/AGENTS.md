# web

The dashboard: a single-page application in TypeScript, built by Vite and embedded in the `cihub` binary by the
`web` role. It reads and acts only through the public API; it holds no queue logic and no rule that the server
does not also check.

## Stack (Q9, decided)

- TypeScript in strict mode (`strict`, `noUncheckedIndexedAccess`); no `any`, no non-null assertion `!`, no
  `@ts-ignore`. Not `exactOptionalPropertyTypes`: the generated client does not compile with it.
- React, Vite, and the TanStack libraries: Router for the routes, Query for the data of the server, Form for the
  forms. Table only for a table that the client sorts or filters; the server already orders the items.
- The client of the API is generated from `web/openapi.json`, the document that `/api/v1/openapi.json` serves,
  by `@hey-api/openapi-ts`, with its plugin `@tanstack/react-query` (`queryOptions`, `mutationOptions`, query
  keys). The generated files are never edited by hand; a change of the API regenerates them in the same change
  (`mise run web-generate`), and `mise run web-generate-check` fails otherwise. The routes outside the API
  (`/auth/*`, `specs/API.md`) are called with `fetch` in `src/session.ts` only.
- Zod 4 for the search parameters and the forms, without adapter (Standard Schema).
- Vitest and Testing Library for the components; Playwright for the browser tests, against a server on the
  simulator (`cihub-scm`, feature `sim`), never the real SCM.
- A new dependency of `package.json` is proposed to the project lead, as a crate is.

## Routes

- File-based routing with `@tanstack/router-plugin/vite`, placed before `@vitejs/plugin-react`, with
  `autoCodeSplitting`. The files of `src/routes/` follow the URL: `/orgs/$org/repos/$repo/queues/$queue`, as the
  API names its resources.
- The URL holds the state that a link must keep: the organization, the queue, the open PR, the tab. Every search
  parameter goes through `validateSearch` with a Zod schema.
- A link is a typed `<Link to=…>` or `navigate({ to })`; never a path built from strings.
- The router context carries the `QueryClient`. The layout of the signed-in pages reads `/me` in `beforeLoad`
  and redirects to the sign-in page on `401`.

## Data of the server

- Every read goes through TanStack Query with the generated `queryOptions`; never a `fetch` by hand, never a copy
  of the data of the server in `useState`, a context or a store.
- A route loader starts its reads with `queryClient.query(...)` (`ensureQueryData` is deprecated); the component
  reads them with `useSuspenseQuery`. No request waterfall: a loader starts the independent reads together.
- The stream `/orgs/{org}/stream` (`src/live.ts`) invalidates the query keys of the queue that changed, and every
  read of the organization when it opens again after a loss; no polling.
- A mutation invalidates the keys that it changes in `onSettled`. No optimistic update of the queue: the API
  answers `202` and the queue decides at its next reconciliation; the page shows the request as sent until the
  stream signals the change.
- An error is the `Problem` of the API: the page decides on its `code`, never on `detail` or on the status alone.
  A route has its `errorComponent`, which resets the queries with `useQueryErrorResetBoundary`.

## Vocabulary and rights

- The page shows `state_description` and the descriptions that the API gives; it decides on the codes
  (`state_reason`, `kind`, `code`). It never writes its own sentence for a reason of the closed vocabulary.
- The role of `/me` only hides or disables a button; the server refuses what the role does not allow.
- Every text is in English, with the words of `specs/DOMAIN.md`. The texts of the dashboard may name GitHub, as
  the mockups do ("Continue with GitHub"), an exception to the neutral vocabulary of the code that the project
  lead accepted: the name will come from the API when a second SCM arrives.

## Interface

- The design tokens (colors, spacing, the two themes) are CSS custom properties, taken from the mockups; no CSS
  framework. A component keeps its styles in a CSS module next to it, with a `*.module.d.css.ts` that names its
  classes, so that a class is a `string` and not `string | undefined`.
- Every action works with the keyboard; `Escape` closes a menu, a drawer or a modal. A page has its
  loading, empty and error states.

## Commands

`mise run lint` and `mise run check` cover the dashboard: type check, ESLint (`typescript-eslint`,
`react-hooks`, `@tanstack/eslint-plugin-query`, `@tanstack/eslint-plugin-router`), Prettier, Vitest.
`mise run web-dev` serves it with Vite in front of a `cihub server start` (`CIHUB_URL`, `http://localhost:8080`);
`mise run web-build` builds `web/dist`, which the next build of `cihub-web` embeds; `mise run web-generate`
writes `openapi.json` from the code (`cargo run -p cihub-cli --example openapi`) and the client in `src/client`;
`mise run e2e` runs the browser tests of `e2e/` (Playwright) against `cargo run -p cihub-cli --example e2e`, a
server on the simulator with a fresh database, which runs the queues, submits PRs of `acme/web` at its start,
passes each test commit of `acme/api`, comments `/cihub merge` on a PR at `/sim/merge`, and gives the last
comment of a PR at `/sim/comment`.

Specs: `specs/API.md`, `specs/SECURITY.md`, `specs/LOTS.md` (lots 10a to 10d).
