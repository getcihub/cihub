import { createFileRoute, redirect } from "@tanstack/react-router";
import { z } from "zod";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { PendingPage } from "@/components/frame/pending-page";
import { deviceAddress } from "@/domain/session/device";
import { readMe } from "@/domain/session/session";
import { DevicePage } from "./page";

const search = z.object({
  code: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/device")({
  validateSearch: search,
  loaderDeps: ({ search: { code } }) => ({ code }),
  loader: async ({ context, deps }) => {
    const me = await readMe(context.queryClient);
    if (!me) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: deviceAddress(deps.code) } });
    }
    return { me };
  },
  component: DevicePage,
  pendingComponent: PendingPage,
  errorComponent: ServerErrorPage,
});
