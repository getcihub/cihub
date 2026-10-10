import { createFileRoute, redirect } from "@tanstack/react-router";

import { readMe } from "../session";

export const Route = createFileRoute("/")({
  beforeLoad: async ({ context }) => {
    const me = await readMe(context.queryClient);
    if (!me || me.orgs.length === 0) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: "/" } });
    }
    return { me };
  },
  component: Home,
});

// The list of the queues replaces this page in lot 10b.
function Home() {
  const { me } = Route.useRouteContext();
  return (
    <main>
      Signed in as @{me.login}: {me.orgs.map((org) => org.slug).join(", ")}.
    </main>
  );
}
