import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { PendingPage } from "@/components/frame/pending-page";
import { pageOfThisSite, readMe } from "@/domain/session/session";
import { SignInPage } from "./page";

const search = z.object({
  error: z.literal("failed").optional().catch(undefined),
  next: pageOfThisSite.optional().catch(undefined),
});

export const Route = createFileRoute("/sign-in")({
  validateSearch: search,
  loaderDeps: ({ search: { next } }) => ({ next }),
  loader: async ({ context, deps }) => {
    const me = await readMe(context.queryClient);
    if (me && me.orgs.length > 0) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ href: deps.next ?? "/" });
    }
    return { me };
  },
  component: SignInPage,
  pendingComponent: PendingPage,
  errorComponent: ServerErrorPage,
});
