import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import { ServerError } from "@/components/errors/server-error";
import { DrawerFrame } from "@/pages/pr/components/drawer-frame";

const route = getRouteApi("/orgs/$org/repos/$repo/queues/$queue/prs/$number");

/** The PR could not be read: the drawer says what the server answered. */
export function PrDrawerError({ error, reset }: ErrorComponentProps) {
  const { org, repo, queue, number } = route.useParams();

  return (
    <DrawerFrame path={{ org, repo, queue }} title={`#${String(number)}`}>
      <ServerError error={error} reset={reset} />
    </DrawerFrame>
  );
}
