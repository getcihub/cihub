import { Link } from "@tanstack/react-router";

import { Button, buttonVariants, Card, EmptyState, LostIllustration, Page } from "@cihub/ui";
import { type Me, membershipOf } from "@/domain/session/session";
import { useSignOut } from "@/hooks/use-sign-out";

/**
 * The API answers 404 on an organization where the person has no role, or
 * that does not exist: the page says so, and offers another organization of
 * the person. An organization that the App left keeps its members: its
 * queues say that the access is lost (lot 10b).
 */
export function OrgLostPage({ me, org }: { me: Me; org: string }) {
  const { startSignOut, hasSignOutFailed } = useSignOut();
  const current = membershipOf(me, org);
  const otherOrg = me.orgs.find((membership) => membership !== current);

  return (
    <Page>
      <Card>
        <EmptyState
          illustration={<LostIllustration />}
          title={`You have no access to ${org}`}
          heading="h1"
          actions={
            <>
              {otherOrg && (
                <Link
                  className={buttonVariants({ variant: "primary" })}
                  to="/orgs/$org"
                  params={{ org: otherOrg.slug }}
                >
                  Open {otherOrg.slug}
                </Link>
              )}
              <Button onClick={startSignOut}>Sign out</Button>
            </>
          }
        >
          This organization does not exist, or your account is not one of its members on GitHub. Ask an owner
          of <b>{org}</b> if this is a mistake.
          {hasSignOutFailed && (
            <span role="alert"> The sign-out failed: reload the page, then try again.</span>
          )}
        </EmptyState>
      </Card>
    </Page>
  );
}
