import { createFileRoute, redirect } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { PendingPage } from "@/components/frame/pending-page";
import { readMe } from "@/domain/session/session";
import { DevicePage } from "./page";

export const Route = createFileRoute("/device")({
  loader: async ({ context }) => {
    const me = await readMe(context.queryClient);
    if (!me) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: "/device" } });
    }
    return { me };
  },
  component: DevicePage,
  pendingComponent: PendingPage,
  errorComponent: ServerErrorPage,
});
