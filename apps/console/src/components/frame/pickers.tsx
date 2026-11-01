import { Link } from "@tanstack/react-router";

import type { Queue } from "@/client/types.gen";
import { UpDownIcon } from "@cihub/icons/up-down";
import { Avatar } from "@cihub/ui/components/avatar";
import { Menu, MenuHint, menuItemVariants, MenuSeparator } from "@cihub/ui/components/menu";
import { cn } from "@cihub/ui/lib/cn";
import { inQueueCount } from "@/domain/queue/counts";
import { type Me, membershipOf } from "@/domain/session/queries";

const PICKER =
  "inline-flex h-7 max-w-full min-w-0 cursor-pointer items-center gap-1 rounded-sm pr-1.5 pl-2 text-foreground hover:bg-muted max-md:pr-1 max-md:pl-1.5 [&_svg]:flex-none [&_svg]:text-muted-foreground";

function PickerLabel({ text, isPlaceholder = false }: { text: string; isPlaceholder?: boolean }) {
  return (
    <>
      <span className={cn("truncate", isPlaceholder && "text-muted-foreground")}>{text}</span>
      <UpDownIcon />
    </>
  );
}

export function OrgPicker({ me, org }: { me: Me; org: string }) {
  const current = membershipOf(me, org);

  return (
    <Menu label="Organization" trigger={<PickerLabel text={org} />} triggerClassName={PICKER} align="left">
      {me.orgs.map((membership) => (
        <Link
          key={membership.slug}
          className={menuItemVariants({ isCurrent: membership === current })}
          to="/orgs/$org"
          params={{ org: membership.slug }}
        >
          <Avatar name={membership.slug} />
          {membership.slug}
          <MenuHint>{membership.role === "admin" ? "Admin" : "Member"}</MenuHint>
        </Link>
      ))}
      {me.app_install_url && (
        <>
          <MenuSeparator />
          <a className={menuItemVariants()} href={me.app_install_url}>
            Add an organization<MenuHint>Install the App</MenuHint>
          </a>
        </>
      )}
    </Menu>
  );
}

export function RepoPicker({
  org,
  names,
  queues,
  repo,
}: {
  org: string;
  names: string[];
  queues: Queue[];
  repo: string | undefined;
}) {
  return (
    <Menu
      label="Repository"
      trigger={<PickerLabel text={repo ?? "All repositories"} isPlaceholder={!repo} />}
      triggerClassName={PICKER}
      align="left"
    >
      <Link className={menuItemVariants({ isCurrent: !repo })} to="/orgs/$org" params={{ org }}>
        All repositories<MenuHint>{names.length}</MenuHint>
      </Link>
      <MenuSeparator />
      {names.map((name) => (
        <RepoLink
          key={name}
          org={org}
          repo={name}
          queues={queues.filter((candidate) => candidate.repo === name)}
          isCurrent={name === repo}
        />
      ))}
    </Menu>
  );
}

/** A repository of the picker, with the number of its queues: its only queue, else its page. */
function RepoLink({
  org,
  repo,
  queues,
  isCurrent,
}: {
  org: string;
  repo: string;
  queues: Queue[];
  isCurrent: boolean;
}) {
  const className = menuItemVariants({ isCurrent });
  const label = (
    <>
      <span>
        <span className="text-muted-foreground">{org}/</span>
        <b className="font-semibold">{repo}</b>
      </span>
      {queues.length > 0 && <MenuHint>{queues.length}</MenuHint>}
    </>
  );
  const [onlyQueue] = queues;

  if (onlyQueue && queues.length === 1) {
    return (
      <Link
        className={className}
        to="/orgs/$org/repos/$repo/queues/$queue"
        params={{ org, repo, queue: onlyQueue.target_branch }}
      >
        {label}
      </Link>
    );
  }

  return (
    <Link className={className} to="/orgs/$org/repos/$repo" params={{ org, repo }}>
      {label}
    </Link>
  );
}

/** The dot of a queue: red for a file that is not valid, green when it runs, amber else. */
function toneOf(queue: Queue): string {
  if (queue.config_error) {
    return "bg-destructive";
  }
  return queue.state === "running" && !queue.is_suspended ? "bg-success" : "bg-warning";
}

export function QueuePicker({
  org,
  repo,
  queues,
  queue,
}: {
  org: string;
  repo: string;
  queues: Queue[];
  queue: string;
}) {
  return (
    <Menu label="Queue" trigger={<PickerLabel text={queue} />} triggerClassName={PICKER} align="left">
      {queues.map((candidate) => (
        <Link
          key={candidate.target_branch}
          className={menuItemVariants({ isCurrent: candidate.target_branch === queue })}
          to="/orgs/$org/repos/$repo/queues/$queue"
          params={{ org, repo, queue: candidate.target_branch }}
        >
          <span className={cn("size-1.75 flex-none rounded-full", toneOf(candidate))} />
          <b className="font-semibold">{candidate.target_branch}</b>
          <MenuHint>
            {candidate.config_error ? "invalid" : `${String(inQueueCount(candidate.counts))} in the queue`}
          </MenuHint>
        </Link>
      ))}
    </Menu>
  );
}
