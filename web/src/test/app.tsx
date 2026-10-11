import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createMemoryHistory, createRouter } from "@tanstack/react-router";
import { render } from "@testing-library/react";
import { vi } from "vitest";

import { client } from "../client/client.gen";
import { routeTree } from "../routeTree.gen";

export type Answer = [number, unknown];
/**
 * The answers of the API by path, or by path and query when the query tells two reads apart; a list answers its
 * items in order, then its last one.
 */
export type Answers = Record<string, Answer | Answer[]>;

export const problem = (status: number, code: string) => ({
  type: "about:blank",
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
    const answer = answers[found] ?? [404, problem(404, "not_found")];
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
      return Promise.resolve(new Response(JSON.stringify(body), { status, headers }));
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
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}
