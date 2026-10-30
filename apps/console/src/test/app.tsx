import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createMemoryHistory, createRouter } from "@tanstack/react-router";
import { render } from "@testing-library/react";
import { vi } from "vitest";

import { client } from "@/client/client.gen";
import type { ErrorCode } from "@/client/types.gen";
import { MotionProvider } from "@cihub/ui/components/motion";
import { sendCsrfToken } from "@/domain/session/session";
import { routeTree } from "@/routeTree.gen";

// As `main.tsx` does, once.
sendCsrfToken();

/** A status and a body, or `null` for an empty body. */
export type Answer = [number, object | null];
/**
 * The answers of the API by path, or by path and query when the query tells two reads apart; a list answers its
 * items in order, then its last one.
 */
export type Answers = Record<string, Answer | Answer[]>;

export const problem = (status: number, code: ErrorCode) => ({
  type: `https://cihub.io/docs/api/errors#${code}`,
  title: code,
  status,
  detail: "",
  code,
});

/** The dashboard at `path`, with the API answering `answers`, and 404 else. */
export function renderApp(path: string, answers: Answers) {
  const calls = new Map<string, number>();
  const answerOf = (url: URL): Answer => {
    const key = `${url.pathname}${decodeURIComponent(url.search)}`;
    const found = answers[key] ? key : url.pathname;
    const answer = answers[found] ?? [404, problem(404, "route_not_found")];
    const call = calls.get(found) ?? 0;
    calls.set(found, call + 1);
    if (typeof answer[0] === "number") {
      return answer as Answer;
    }
    const list = answer as Answer[];
    return list[Math.min(call, list.length - 1)] ?? [500, null];
  };

  // jsdom has no address for a relative URL of `fetch`.
  client.setConfig({ baseUrl: "http://localhost/api/v1" });
  vi.stubGlobal(
    "fetch",
    vi.fn((input: Request | string) => {
      const [status, body] = answerOf(new URL(typeof input === "string" ? input : input.url));
      const headers = { "content-type": "application/json" };
      // A `204` carries no body: `Response` refuses one.
      const text = status === 204 ? null : JSON.stringify(body);
      return Promise.resolve(new Response(text, { status, headers }));
    }),
  );
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createRouter({
    routeTree,
    context: { queryClient },
    history: createMemoryHistory({ initialEntries: [path] }),
  });

  render(
    <QueryClientProvider client={queryClient}>
      <MotionProvider skipAnimations>
        <RouterProvider router={router} />
      </MotionProvider>
    </QueryClientProvider>,
  );
  return router;
}

/** Gives the session the CSRF cookie that the server sets; a script only reads it. */
export function mockSessionCookie(): void {
  vi.spyOn(document, "cookie", "get").mockReturnValue("__Host-cihub_csrf=token");
}

/** The requests of the page that change a state: method, path, body and CSRF token. */
export async function sentChanges(): Promise<[string, string, object | null, string | null][]> {
  const calls = vi.mocked(fetch).mock.calls.map(([input]) => input as Request);
  const sent = calls.filter((request) => request.method !== "GET");
  return Promise.all(
    sent.map(async (request) => [
      request.method,
      new URL(request.url).pathname,
      JSON.parse((await request.clone().text()) || "null") as object | null,
      request.headers.get("X-CSRF-Token"),
    ]),
  );
}
