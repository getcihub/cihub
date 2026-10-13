import { useQueryClient, useQueryErrorResetBoundary } from "@tanstack/react-query";
import { type ErrorComponentProps, useLocation, useNavigate, useRouter } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { FailureIllustration } from "@/components/ui/illustrations";
import { failureOf, isProblem, problemOf } from "@/domain/problem";
import { forgetMe, readMe } from "@/domain/session/session";

interface Props extends Pick<ErrorComponentProps, "error" | "reset"> {
  /** `h1` when the error is the whole page; `h3` in a page, under its head. */
  heading?: "h1" | "h3";
}

/**
 * A read of the page that failed: what the server answered, and "Try again". The queues keep working: only this
 * page could not read them. A session that ended meanwhile goes back to the sign-in instead.
 */
export function ServerError(props: Props) {
  if (isProblem(failureOf(props.error), "unauthorized")) {
    return <SignInAgain {...props} />;
  }

  return <FailedRead {...props} />;
}

function FailedRead({ error, reset, heading = "h3" }: Props) {
  const router = useRouter();
  const queries = useQueryErrorResetBoundary();
  const problem = problemOf(failureOf(error));
  // As `503 Service Unavailable`.
  const answer = problem && `${String(problem.status)} ${problem.title}`;

  return (
    <EmptyState
      illustration={<FailureIllustration />}
      title="CIHub could not load this page"
      heading={heading}
      actions={
        <Button
          variant="primary"
          onClick={() => {
            queries.reset();
            reset();
            void router.invalidate();
          }}
        >
          Try again
        </Button>
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
  );
}

/**
 * A read answered 401: the sign-in, then back to this page, once a new read of the person says that the session
 * ended. A session that still holds shows the failure instead, so that the sign-in never sends the person back
 * here in a loop.
 */
function SignInAgain(props: Props) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  // The page that failed, as the first render saw it: during the navigation to the sign-in, the location is
  // already the one of the sign-in.
  const [next] = useState(useLocation().href);
  const [hasSession, setHasSession] = useState(false);

  useEffect(() => {
    forgetMe(queryClient);
    void readMe(queryClient).then(
      (me) => {
        if (me) {
          setHasSession(true);
          return;
        }
        void navigate({ to: "/sign-in", search: { next }, replace: true });
      },
      () => {
        setHasSession(true);
      },
    );
  }, [queryClient, navigate, next]);

  return hasSession ? <FailedRead {...props} /> : null;
}
