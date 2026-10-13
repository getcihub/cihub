import { createFileRoute, redirect } from "@tanstack/react-router";

/** `/orgs` lists nothing: `/` opens the organization that the person opened last. */
export const Route = createFileRoute("/orgs/")({
  beforeLoad: () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
    throw redirect({ to: "/" });
  },
});
