import { Link } from "@tanstack/react-router";

import { buttonVariants, Card, EmptyState, LostIllustration, Page } from "@cihub/ui";

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
