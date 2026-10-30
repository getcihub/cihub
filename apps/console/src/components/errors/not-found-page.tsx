import { Link } from "@tanstack/react-router";

import { buttonVariants } from "@cihub/ui/components/button";
import { Card } from "@cihub/ui/components/card";
import { EmptyState } from "@cihub/ui/components/empty-state";
import { LostIllustration } from "@cihub/ui/components/illustrations";
import { Page } from "@cihub/ui/components/page";

/** An address that leads nowhere in CIHub. */
export function NotFoundPage() {
  return (
    <Page>
      <Card>
        <EmptyState
          illustration={<LostIllustration />}
          title="Page not found"
          heading="h1"
          actions={
            <Link className={buttonVariants()} to="/">
              Back to the queues
            </Link>
          }
        >
          This address leads nowhere in CIHub.
        </EmptyState>
      </Card>
    </Page>
  );
}
