import type { QueryClient } from "@tanstack/react-query";
import { Outlet, createRootRouteWithContext } from "@tanstack/react-router";

import { NotFoundPage } from "@/components/errors/NotFoundPage";
import { ServerErrorPage } from "@/components/errors/ServerErrorPage";

export interface RouterContext {
  queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: Outlet,
  notFoundComponent: NotFoundPage,
  errorComponent: ServerErrorPage,
});
