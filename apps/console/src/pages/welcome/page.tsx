import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect } from "react";

import { syncUserMutation } from "@/client/@tanstack/react-query.gen";
import type { OrgList } from "@/client/types.gen";
import { ArrowIcon } from "@cihub/icons/arrow";
import { Avatar } from "@cihub/ui/components/avatar";
import { Button, buttonVariants } from "@cihub/ui/components/button";
import { COMMAND } from "@/domain/queue/words";
import { ErrorAlert } from "@/components/errors/error-alert";
import { Brand, Note } from "@/components/session/parts";
import { invalidateUserOrgs } from "@/domain/session/actions";
import { userOrgsOptions } from "@/domain/session/queries";
import { useMe } from "@/hooks/use-me";
import { useReadAgain } from "@/hooks/use-read-again";
import { useSignOut } from "@/hooks/use-sign-out";
import { Step } from "./components/step";

/**
 * The sign-out page of the GitHub of the App, next to its installation page: GitHub asks which account to use
 * only when no account is signed in (`prompt=select_account` does nothing for a GitHub App).
 */
function signOutOf(installUrl: string | null | undefined): string | null {
  return installUrl ? `${new URL(installUrl).origin}/logout` : null;
}

/**
 * A person signed in without an organization: CIHub runs only where its App is installed, so the page leads
 * there. "Check again" reads the installations of the person on GitHub, then opens the first organization.
 */
export function WelcomePage() {
  const navigate = useNavigate();

  const me = useMe();
  const queryClient = useQueryClient();
  const wait = useReadAgain((data: OrgList | undefined) => !data?.orgs.length, false);
  const orgs = useQuery({ ...userOrgsOptions(), refetchInterval: wait.refetchInterval });
  const sync = useMutation({
    ...syncUserMutation(),
    onSettled: () => invalidateUserOrgs(queryClient),
    onSuccess: wait.restart,
  });
  const { startSignOut, hasSignOutFailed } = useSignOut();

  const first = orgs.data?.orgs[0];
  const isChecking = wait.hasStarted && !wait.isOver && !first;
  const signOutUrl = signOutOf(me.app_install_url);

  useEffect(() => {
    if (first) {
      void navigate({ to: "/orgs/$org", params: { org: first.slug } });
    }
  }, [first, navigate]);

  return (
    <div className="min-h-screen bg-background">
      <header className="flex items-center justify-between border-b border-divider px-6 py-3">
        <Brand className="text-xl" />
        <div className="flex items-center gap-2 text-md text-secondary-foreground">
          <Avatar name={me.login} src={me.avatar_url} size={24} />@{me.login}
          <Button onClick={startSignOut}>Sign out</Button>
        </div>
      </header>
      <main className="mx-auto flex max-w-160 flex-col gap-7 px-6 pt-14 pb-20">
        <div>
          <h1 className="mb-2 text-5xl font-semibold tracking-[-0.5px]">Welcome to CIHub, @{me.login}</h1>
          <p className="text-lg text-secondary-foreground">
            CIHub runs on the organizations where its GitHub App is installed. Your account has none yet:
            three steps lead to your first merge.
          </p>
        </div>
        {hasSignOutFailed && (
          <ErrorAlert className="mb-0">The sign-out failed. Reload the page, then try again.</ErrorAlert>
        )}
        <ol className="flex flex-col gap-3">
          <Step n={1} title="Install the App on an organization" isNow>
            <p>
              Choose the organization and its repositories on GitHub. GitHub brings you back here once it is
              done.
            </p>
            {me.app_install_url && (
              <div className="mt-3 flex gap-2">
                <a className={buttonVariants({ variant: "primary" })} href={me.app_install_url}>
                  Install the App on GitHub <ArrowIcon />
                </a>
              </div>
            )}
            <Note>
              Only an owner of the organization can install it; a member sends a request to the owners. An
              owner already installed it?{" "}
              <Button
                className="inline-flex"
                isPending={sync.isPending || isChecking}
                pendingText="Checking"
                onClick={() => {
                  sync.mutate({});
                }}
              >
                Check again
              </Button>
            </Note>
            {sync.isError && (
              <ErrorAlert className="mt-2 mb-0">The check failed. Try again in a moment.</ErrorAlert>
            )}
            {wait.isOver && !first && (
              <Note>No organization with the App yet: install it, or ask an owner to.</Note>
            )}
            <Note>
              To use another GitHub account,{" "}
              {signOutUrl ? <a href={signOutUrl}>sign out of GitHub</a> : "sign out of GitHub"}, then sign out
              here and sign in again.
            </Note>
          </Step>
          <Step n={2} title="Check the branch rules">
            <p>CIHub reads the rulesets of each repository and says what to change, if anything.</p>
          </Step>
          <Step n={3} title="Start the first queue">
            <p>
              Comment <code>{COMMAND}</code> on an approved pull request. The queue starts there.
            </p>
          </Step>
        </ol>
      </main>
    </div>
  );
}
