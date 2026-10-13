import { useState } from "react";

import { signInAddress } from "@/domain/session/session";
import { buttonVariants } from "@/ui/button";
import { ArrowIcon } from "@/ui/icons";
import { Spinner } from "@/ui/spinner";
import { Failure, Lead, Note, Title } from "./parts";

/** The sign-in through the SCM: then CIHub opens `next`. */
export function SignInForm({ hasFailed, next }: { hasFailed: boolean; next: string }) {
  const [isLeaving, setIsLeaving] = useState(false);
  return (
    <>
      <Title>Sign in</Title>
      <Lead>See and drive the merge queues of your organizations.</Lead>
      {hasFailed && <Failure className="mb-5">The sign-in failed. Try again.</Failure>}
      <a
        className={buttonVariants({ variant: "primary", size: "lg" })}
        href={signInAddress(next)}
        aria-disabled={isLeaving}
        onClick={() => {
          setIsLeaving(true);
        }}
      >
        {isLeaving ? (
          <>
            <Spinner className="size-4" />
            Redirecting to GitHub
          </>
        ) : (
          <>
            Continue with GitHub <ArrowIcon />
          </>
        )}
      </a>
      <Note>
        CIHub asks GitHub who you are and which organizations of its App you can reach. It does not keep your
        GitHub token. A session lasts 14 days.
      </Note>
    </>
  );
}
