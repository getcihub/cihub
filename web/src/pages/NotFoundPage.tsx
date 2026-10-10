import { Link } from "@tanstack/react-router";

import { EmptyState } from "../ui/EmptyState";
import { LostIllustration } from "../ui/illustrations";
import styles from "../ui/ui.module.css";

/** An address that leads nowhere in CIHub. */
export function NotFoundPage() {
  return (
    <main className={styles.page}>
      <div className={styles.card}>
        <EmptyState
          illustration={<LostIllustration />}
          title="Page not found"
          heading="h1"
          actions={
            <Link className={styles.button} to="/">
              Back to the queues
            </Link>
          }
        >
          This address leads nowhere in CIHub.
        </EmptyState>
      </div>
    </main>
  );
}
