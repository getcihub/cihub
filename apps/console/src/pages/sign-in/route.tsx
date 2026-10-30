import { createFileRoute, redirect } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { PendingPage } from "@/components/frame/pending-page";
import { readMe } from "@/domain/session/queries";
import { signInSearch } from "@/domain/session/search";
import { SignInPage } from "./page";

export const Route = createFileRoute("/sign-in")({
  validateSearch: signInSearch,
  loaderDeps: ({ search: { next } }) => ({ next }),
  loader: async ({ context, deps }) => {
    const me = await readMe(context.queryClient);
    if (me) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ href: me.orgs.length > 0 ? (deps.next ?? "/") : "/welcome" });
    }
  },
  component: SignInPage,
  pendingComponent: PendingPage,
  errorComponent: ServerErrorPage,
});
