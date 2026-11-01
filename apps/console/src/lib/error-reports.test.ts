import { afterEach, describe, expect, it, vi } from "vitest";

import { startErrorReports } from "./error-reports";

interface Options {
  // eslint-disable-next-line no-restricted-syntax -- the SDK gives what was thrown, whatever it is
  beforeSend: (event: object, hint: { originalException?: unknown }) => object | null;
}
const sentry = vi.hoisted(() => ({
  init: vi.fn<(options: Options) => void>(),
  reactErrorHandler: vi.fn(() => vi.fn()),
}));
vi.mock("@sentry/react", () => sentry);

afterEach(() => {
  document.head.replaceChildren();
  vi.clearAllMocks();
});

describe("the error reports of the dashboard", () => {
  it("send nothing when the server gives no DSN", async () => {
    const options = await startErrorReports(() => false);

    expect(options).toEqual({});
    expect(sentry.init).not.toHaveBeenCalled();
  });

  it("go to the DSN that the server writes, and drop the errors that the caller expects", async () => {
    const meta = document.createElement("meta");
    meta.name = "sentry-dsn";
    meta.content = "https://key@sentry.example.com/2";
    document.head.append(meta);

    const options = await startErrorReports((thrown) => thrown === "expected");

    const init = sentry.init.mock.calls[0]?.[0];
    const event = {};
    expect(sentry.init).toHaveBeenCalledWith(expect.objectContaining({ dsn: meta.content }));
    expect(init?.beforeSend(event, { originalException: "expected" })).toBeNull();
    expect(init?.beforeSend(event, { originalException: "other" })).toBe(event);
    expect(options.onCaughtError).toBeDefined();
    expect(options.onUncaughtError).toBeDefined();
  });
});
