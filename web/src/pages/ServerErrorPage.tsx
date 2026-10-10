import type { ErrorComponentProps } from "@tanstack/react-router";
import { useQueryErrorResetBoundary } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import { EmptyState } from "../ui/EmptyState";
import { FailureIllustration } from "../ui/illustrations";
import styles from "../ui/ui.module.css";

/** The problem that the API answered, as `503 Service Unavailable`; `null` for another failure. */
function answerOf(error: unknown): string | null {
  if (typeof error === "object" && error !== null && "status" in error && "title" in error) {
    return `${String(error.status)} ${String(error.title)}`;
  }
  return null;
}

/** A page whose reads failed: the queues keep working, only this page could not read them. */
export function ServerErrorPage({ error, reset }: ErrorComponentProps) {
  const router = useRouter();
  const queries = useQueryErrorResetBoundary();
  const answer = answerOf(error);

  return (
    <main className={styles.page}>
      <div className={styles.card}>
        <EmptyState
          illustration={<FailureIllustration />}
          title="CIHub could not load this page"
          heading="h1"
          actions={
            <button
              type="button"
              className={`${styles.button} ${styles.primary}`}
              onClick={() => {
                queries.reset();
                reset();
                void router.invalidate();
              }}
            >
              Try again
            </button>
          }
        >
          {answer ? (
            <>
              The server answered <code>{answer}</code>.
            </>
          ) : (
            "The server did not answer, or the page failed."
          )}{" "}
          The queues keep working: only this page could not read them.
        </EmptyState>
      </div>
    </main>
  );
}
