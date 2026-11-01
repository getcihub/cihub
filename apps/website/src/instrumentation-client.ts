import posthog from "posthog-js";

// Only the production deployment of Vercel holds the token: the local site and
// the previews send nothing.
const projectToken = process.env.NEXT_PUBLIC_POSTHOG_PROJECT_TOKEN;

if (projectToken) {
  posthog.init(projectToken, {
    // Through the rewrites of next.config.ts: a browser that blocks the
    // domain of PostHog still reaches the site.
    api_host: "/ingest",
    ui_host: "https://eu.posthog.com",
    defaults: "2026-08-30",
    // No cookie and no storage, so no consent banner. The project of PostHog
    // must enable cookieless mode, or it drops these events.
    cookieless_mode: "always",
  });
}
