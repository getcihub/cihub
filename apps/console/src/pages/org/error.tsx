import type { ErrorComponentProps } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { TopBar, TopBarLogo } from "@/components/frame/top-bar";

/** The person or the queues of the organization could not be read: no frame to show, only what the server answered. */
export function OrgLayoutError(props: ErrorComponentProps) {
  return (
    <>
      <TopBar>
        <TopBarLogo />
      </TopBar>
      <ServerErrorPage {...props} />
    </>
  );
}
