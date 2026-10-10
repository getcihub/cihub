import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { pageOfThisSite, readMe } from "../session";
import { SignInPage } from "../sign-in/SignInPage";

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
  component: SignInRoute,
});

function SignInRoute() {
  const { me } = Route.useLoaderData();
  const { error, next } = Route.useSearch();
  return <SignInPage me={me} hasFailed={error === "failed"} next={next ?? "/"} />;
}
