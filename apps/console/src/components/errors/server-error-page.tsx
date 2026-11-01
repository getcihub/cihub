import type { ErrorComponentProps } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Card } from "@cihub/ui/components/card";
import { Page } from "@cihub/ui/components/page";
import { ServerError } from "./server-error";

/**
 * A page whose reads failed: its `head`, or its `band` under the top bar, when it can still show it, then what the
 * server answered. Without either (the sign-in, the frame), the failure is the whole page.
 */
export function ServerErrorPage({
  error,
  reset,
  head,
  band,
}: ErrorComponentProps & { head?: ReactNode; band?: ReactNode }) {
  return (
    <>
      {band}
      <Page>
        {head}
        <Card>
          <ServerError error={error} reset={reset} heading={head || band ? "h3" : "h1"} />
        </Card>
      </Page>
    </>
  );
}
