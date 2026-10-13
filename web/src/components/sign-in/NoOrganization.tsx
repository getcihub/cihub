import { useSignOut } from "@/domain/session/session";
import { Button, buttonVariants } from "@/ui/button";
import { ArrowIcon } from "@/ui/icons";
import { Failure, Lead, Note, Title } from "./parts";

/**
 * The sign-out page of the GitHub of the App, next to its installation page: GitHub asks which account to use
 * only when no account is signed in (`prompt=select_account` does nothing for a GitHub App).
 */
function signOutOf(installUrl: string | null): string | null {
  return installUrl ? `${new URL(installUrl).origin}/logout` : null;
}

/** A user whose account is in no organization of the App: the way out is to install it. */
export function NoOrganization({ installUrl }: { installUrl: string | null }) {
  const signOutUrl = signOutOf(installUrl);
  const { startSignOut, hasSignOutFailed } = useSignOut();
  return (
    <>
      <Title>No organization yet</Title>
      <Lead>Your GitHub account is not a member of an organization where the CIHub App is installed.</Lead>
      {installUrl && (
        <a className={buttonVariants({ variant: "primary", size: "lg" })} href={installUrl}>
          Install the App on GitHub <ArrowIcon />
        </a>
      )}
      <Note>
        Only an owner of the organization can install it. If you are not one, ask an owner, then sign in
        again.
      </Note>
      <Note>
        To use another GitHub account,{" "}
        {signOutUrl ? <a href={signOutUrl}>sign out of GitHub</a> : "sign out of GitHub"}, then sign out here
        and sign in again.
      </Note>
      <Button size="lg" className="mt-5" onClick={startSignOut}>
        Sign out
      </Button>
      {hasSignOutFailed && (
        <Failure className="mt-5">The sign-out failed. Reload the page, then try again.</Failure>
      )}
    </>
  );
}
