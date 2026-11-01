import type { RootOptions } from "react-dom/client";

// The server writes these settings into index.html when the instance has a Sentry server for the dashboard.
function setting(name: string): string | undefined {
  return document.querySelector<HTMLMetaElement>(`meta[name="${name}"]`)?.content;
}

// What the browser or an extension throws, not CIHub.
const IGNORED_ERRORS = [
  "AbortError: Fetch is aborted",
  "The operation was aborted",
  "signal is aborted without reason",
  "The user aborted a request",
  "NotFoundError: Failed to execute 'removeChild' on 'Node'",
  "NotFoundError: Failed to execute 'insertBefore' on 'Node'",
];

/**
 * Starts the error reports and gives the options of the React root that send the errors of the components.
 * Without a DSN, it sends nothing and does not load the SDK. `isExpected` tells the errors to drop.
 */
// eslint-disable-next-line no-restricted-syntax -- a thrown value has no type
export async function startErrorReports(isExpected: (thrown: unknown) => boolean): Promise<RootOptions> {
  const dsn = setting("sentry-dsn");
  if (!dsn) {
    return {};
  }

  const Sentry = await import("@sentry/react");
  Sentry.init({
    dsn,
    environment: setting("sentry-environment"),
    release: setting("sentry-release"),
    allowUrls: [window.location.origin],
    ignoreErrors: IGNORED_ERRORS,
    beforeSend: (event, hint) => (isExpected(hint.originalException) ? null : event),
  });
  // A handler of the root replaces the one of React, which writes the error in the console.
  const report = Sentry.reactErrorHandler((error) => {
    console.error(error);
  });
  return { onCaughtError: report, onUncaughtError: report };
}
