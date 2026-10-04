# Dashboard (`web/`)

- React, TypeScript, Vite, pnpm (D173). Every dependency is a decision; `pnpm add` respects the 3 days of
  `minimumReleaseAge`, never bypass it with `minimumReleaseAgeExclude`. The lockfile `web/pnpm-lock.yaml` changes
  with `package.json`.
- The app talks to the cihub server only through `/api/v1`. It runs without it (D175): a browser test or a
  screenshot scenario mocks the API routes it reads with `page.route`, through `e2e/fixtures.ts`.
- The paths of the app must not start with `/api/`, `/assets/` or `/webhooks/`: `cihub-web` keeps them (D174).
- Tests: Vitest with Testing Library for the components (`src/**/*.test.tsx`), Playwright for the browser
  (`e2e/*.spec.ts`). `mise run web-lint`, `mise run web-test`, then `mise run web-e2e`; `mise run web-embed` when
  `crates/web` or the build changes.
- Visual change: add or update its scenario in `e2e/shots.spec.ts` (fixed clock, mocked API), run
  `mise run web-shots-compare`, look at `web/.screenshots/compare.html` and show the before and after images to
  the person.
- Documents: `docs/DEPLOYMENT.md` (tasks, image), `docs/API.md` (the routes the app reads).
