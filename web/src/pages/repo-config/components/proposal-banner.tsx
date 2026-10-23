import type { ConfigProposal } from "@/client/types.gen";
import { ArrowIcon } from "@/components/icons/arrow-icon";
import { WarningIcon } from "@/components/icons/warning-icon";
import { Alert, AlertContent, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Prose } from "@/components/ui/prose";

/** The last proposal of a change of the file that has not merged: CIHub opens it, it waits for a merge, or it failed. */
export function ProposalBanner({ proposal, branch }: { proposal: ConfigProposal; branch: string }) {
  if (proposal.state === "failed") {
    return (
      <Alert variant="destructive" isBanner>
        <WarningIcon />
        <AlertContent>
          <AlertTitle>The proposal failed:</AlertTitle>
          <AlertDescription>
            <Prose text={proposal.state_description ?? ""} />.
          </AlertDescription>
        </AlertContent>
      </Alert>
    );
  }

  if (proposal.state === "requested" || proposal.pr_number == null) {
    return (
      <Alert role="status" isBanner>
        <ArrowIcon />
        <AlertContent>
          <AlertTitle>CIHub opens the pull request.</AlertTitle>
          <AlertDescription>
            It writes the file on <code>cihub-config/{branch}</code>, then opens a pull request into{" "}
            <code>{branch}</code>.
          </AlertDescription>
        </AlertContent>
      </Alert>
    );
  }

  return (
    <Alert role="status" isBanner>
      <ArrowIcon />
      <AlertContent className="flex-1">
        <AlertTitle>Pull request #{proposal.pr_number} changes this file.</AlertTitle>
        <AlertDescription>
          The queues take the new settings once it merges on <code>{branch}</code>.
        </AlertDescription>
      </AlertContent>
      {proposal.pr_url && (
        <a className="text-base font-medium text-primary hover:underline" href={proposal.pr_url}>
          Open #{proposal.pr_number} on GitHub
        </a>
      )}
    </Alert>
  );
}
