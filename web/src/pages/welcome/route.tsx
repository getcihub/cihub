import { createFileRoute, redirect } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { PendingPage } from "@/components/frame/pending-page";
import { readMe } from "@/domain/session/session";
import { WelcomePage } from "./page";

/** The page of a person without an organization; the others open their organization, and nobody signs in here. */
export const Route = createFileRoute("/welcome")({
  beforeLoad: async ({ context }) => {
    const me = await readMe(context.queryClient);
    if (!me) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: "/welcome" } });
    }
    if (me.orgs.length > 0) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/" });
    }
  },
  component: WelcomePage,
  pendingComponent: PendingPage,
  errorComponent: ServerErrorPage,
});
