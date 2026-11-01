import type { RootOptions } from "react-dom/client";

// The server writes these settings into index.html when the instance has a Sentry server for the dashboard
// (specs/OBSERVABILITY.md).
function setting(name: string): string | undefined {
  return document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`)?.content;
}

/**
 * Starts the error reports, and gives the options of the React root that send the errors of the components.
 * Without a DSN, the dashboard sends nothing and does not load the SDK.
 */
export async function startErrorReports(): Promise<RootOptions> {
  const dsn = setting("sentry-dsn");
  if (!dsn) {
    return {};
  }

  const Sentry = await import("@sentry/react");
  Sentry.init({ dsn, environment: setting("sentry-environment"), release: setting("sentry-release") });
  // A handler of the root replaces the one of React, which writes the error in the console.
  const report = Sentry.reactErrorHandler((error) => {
    console.error(error);
  });
  return { onCaughtError: report, onUncaughtError: report };
}
