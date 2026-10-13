import { Link } from "@tanstack/react-router";

import { EmptyState } from "@/ui/empty-state";
import { LostIllustration } from "@/ui/illustrations";
import { buttonVariants } from "@/ui/button";
import { Card } from "@/ui/card";
import { Page } from "@/ui/page";

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
