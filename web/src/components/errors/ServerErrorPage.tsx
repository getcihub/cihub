import type { ErrorComponentProps } from "@tanstack/react-router";

import { Card } from "@/ui/card";
import { Page } from "@/ui/page";
import { ServerError } from "./ServerError";

/** A page whose reads failed before it could show anything of its own: the sign-in, the frame. */
export function ServerErrorPage({ error, reset }: ErrorComponentProps) {
  return (
    <Page>
      <Card>
        <ServerError error={error} reset={reset} heading="h1" />
      </Card>
    </Page>
  );
}
