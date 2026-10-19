import type { RepoDetail, RuleCheck } from "@/client/types.gen";
import { CheckIcon } from "@/components/icons/check-icon";
import { ErrorIcon } from "@/components/icons/error-icon";
import { WarningIcon } from "@/components/icons/warning-icon";
import { Alert, AlertContent, AlertTitle } from "@/components/ui/alert";
import { Prose } from "@/components/ui/prose";
import { cn } from "@/lib/cn";

/** The four rules that a queue needs, as the server read them; the link to change them on the SCM. */
export function SetupRules({ repo, isChecking }: { repo: RepoDetail; isChecking: boolean }) {
  const { setup } = repo;

  if (setup.target_branch_missing) {
    return (
      <Alert isBanner variant="destructive" role="status">
        <ErrorIcon />
        <AlertContent>
          <AlertTitle>
            <code>{repo.default_branch}</code> has no commit yet. Push a first commit, then CIHub can read its
            rules.
          </AlertTitle>
        </AlertContent>
      </Alert>
    );
  }
  if (setup.rules.length === 0) {
    return (
      <p className="text-md text-muted-foreground">{isChecking ? "Reading the rules…" : "Not read yet."}</p>
    );
  }

  return (
    <>
      <ul className="flex flex-col gap-2">
        {setup.rules.map((rule) => (
          <Rule key={rule.code} rule={rule} />
        ))}
      </ul>
      {setup.rules.some((rule) => !rule.passed) && setup.rules_url && (
        <a className="mt-3 inline-block text-md" href={setup.rules_url}>
          Open the rules of {repo.name} on GitHub →
        </a>
      )}
    </>
  );
}

function Rule({ rule }: { rule: RuleCheck }) {
  const tone = rule.passed ? "text-success" : rule.blocks_merge ? "text-destructive" : "text-warning";
  const icon = rule.passed ? <CheckIcon /> : rule.blocks_merge ? <ErrorIcon /> : <WarningIcon />;

  return (
    <li className="text-md">
      <div className="flex items-center gap-2">
        <span className={cn("flex-none [&>svg]:size-4", tone)}>{icon}</span>
        <span>
          <Prose text={rule.title} />
        </span>
        {!rule.passed && (
          <span className="text-sm text-muted-foreground">
            {rule.blocks_merge ? "blocks every merge" : "warning"}
          </span>
        )}
      </div>
      {rule.message && (
        <p className="ml-6 text-md text-secondary-foreground">
          <Prose text={rule.message} />
        </p>
      )}
    </li>
  );
}
