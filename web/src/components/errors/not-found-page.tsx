import { Link } from "@tanstack/react-router";

import { buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { LostIllustration } from "@/components/ui/illustrations";
import { Page } from "@/components/ui/page";

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
