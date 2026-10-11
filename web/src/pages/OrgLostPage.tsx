import { Link } from "@tanstack/react-router";
import { useState } from "react";

import type { Me } from "../session";
import { signOutAndLeave } from "../session";
import { EmptyState } from "../ui/EmptyState";
import { LostIllustration } from "../ui/illustrations";
import styles from "../ui/ui.module.css";

/**
 * The API answers 404 on an organization where the person has no role, or
 * that does not exist: the page says so, and offers another organization of
 * the person. An organization that the App left keeps its members: its
 * queues say that the access is lost (lot 10b).
 */
export function OrgLostPage({ me, org }: { me: Me; org: string }) {
  const [hasSignOutFailed, setHasSignOutFailed] = useState(false);
  const otherOrg = me.orgs.find((membership) => membership.slug.toLowerCase() !== org.toLowerCase());

  return (
    <main className={styles.page}>
      <div className={styles.card}>
        <EmptyState
          illustration={<LostIllustration />}
          title={`You have no access to ${org}`}
          heading="h1"
          actions={
            <>
              {otherOrg && (
                <Link
                  className={`${styles.button} ${styles.primary}`}
                  to="/orgs/$org"
                  params={{ org: otherOrg.slug }}
                >
                  Open {otherOrg.slug}
                </Link>
              )}
              <button
                type="button"
                className={styles.button}
                onClick={() => {
                  void signOutAndLeave().then((hasSignedOut) => {
                    setHasSignOutFailed(!hasSignedOut);
                  });
                }}
              >
                Sign out
              </button>
            </>
          }
        >
          This organization does not exist, or your account is not one of its members on GitHub. Ask an owner
          of <b>{org}</b> if this is a mistake.
          {hasSignOutFailed && (
            <span role="alert"> The sign-out failed: reload the page, then try again.</span>
          )}
        </EmptyState>
      </div>
    </main>
  );
}
