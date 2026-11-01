import { afterEach, describe, expect, it, vi } from "vitest";

import { startErrorReports } from "./error-reports";

const sentry = vi.hoisted(() => ({ init: vi.fn(), reactErrorHandler: vi.fn(() => vi.fn()) }));
vi.mock("@sentry/react", () => sentry);

function addSetting(name: string, content: string) {
  const meta = document.createElement("meta");
  meta.name = name;
  meta.content = content;
  document.head.append(meta);
}

afterEach(() => {
  document.head.replaceChildren();
  vi.clearAllMocks();
});

describe("the error reports of the dashboard", () => {
  it("send nothing when the server gives no DSN", async () => {
    const options = await startErrorReports();

    expect(options).toEqual({});
    expect(sentry.init).not.toHaveBeenCalled();
  });

  it("go to the Sentry server that the server names, with the version of CIHub", async () => {
    addSetting("sentry-dsn", "https://key@sentry.example.com/2");
    addSetting("sentry-release", "cihub@1.2.3");

    const options = await startErrorReports();

    expect(sentry.init).toHaveBeenCalledWith({
      dsn: "https://key@sentry.example.com/2",
      environment: undefined,
      release: "cihub@1.2.3",
    });
    expect(options.onCaughtError).toBeDefined();
    expect(options.onUncaughtError).toBeDefined();
  });
});
