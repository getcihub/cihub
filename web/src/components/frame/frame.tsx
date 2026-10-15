import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Queue } from "@/client/types.gen";
import { UpDownIcon } from "@/components/icons/up-down-icon";
import { Avatar } from "@/components/ui/avatar";
import { Mark } from "@/components/ui/mark";
import { Menu, MenuHint, MenuItem, MenuSeparator, menuItemVariants } from "@/components/ui/menu";
import { inQueueCount } from "@/domain/queue/counts";
import { type Me, membershipOf } from "@/domain/session/session";
import { useSignOut } from "@/hooks/use-sign-out";
import { cn } from "@/lib/cn";
import { toggleTheme } from "@/lib/preferences";

interface Props {
  me: Me;
  /** The organization of the URL. */
  org: string;
  /** Its queues, by repository then name; empty once access is lost. */
  queues: Queue[];
  /** The repository and the queue of the URL, if any. */
  repo?: string | undefined;
  queue?: string | undefined;
  children: ReactNode;
}

const PICKER =
  "inline-flex h-7 max-w-full min-w-0 cursor-pointer items-center gap-1 rounded-sm pr-1.5 pl-2 text-foreground hover:bg-muted max-md:pr-1 max-md:pl-1.5 [&_svg]:flex-none [&_svg]:text-muted-foreground";

/** The bar at the top of a page, with the logo of CIHub first. */
export function TopBar({ children }: { children: ReactNode }) {
  return (
    <header className="sticky top-0 z-10 flex h-13 items-center gap-1 border-b border-border bg-card px-page max-md:gap-0.5 max-md:px-3.5">
      {children}
    </header>
  );
}

/** The logo of the top bar: a link to the queues of `org`, or the logo alone while a page loads. */
export function TopBarLogo({ org }: { org?: string }) {
  const place = "mr-2 grid place-items-center max-md:mr-0.5";
  const logo = <Mark className="size-6" />;

  if (!org) {
    return <span className={place}>{logo}</span>;
  }

  return (
    <Link className={place} to="/orgs/$org" params={{ org }} aria-label="CIHub">
      {logo}
    </Link>
  );
}

/** The top bar of every page of an organization: CIHub / organization / repository / queue. */
export function Frame({ me, org, queues, repo, queue, children }: Props) {
  const { startSignOut, hasSignOutFailed } = useSignOut();
  // The frame names only a repository and a queue that exist: a wrong address shows "not found" under it.
  const knownRepo = queues.some((candidate) => candidate.repo === repo) ? repo : undefined;
  const queuesOfRepo = queues.filter((candidate) => candidate.repo === knownRepo);
  const knownQueue = queuesOfRepo.some((candidate) => candidate.name === queue) ? queue : undefined;

  return (
    <>
      <TopBar>
        <TopBarLogo org={org} />
        <nav className="flex min-w-0 items-center gap-0.5 font-medium" aria-label="Where you are">
          <OrgPicker me={me} org={org} />
          {queues.length > 0 && (
            <>
              <span className="text-waiting">/</span>
              <RepoPicker org={org} queues={queues} repo={knownRepo} />
            </>
          )}
          {knownRepo && knownQueue && (
            <>
              <span className="text-waiting">/</span>
              <QueuePicker org={org} repo={knownRepo} queues={queuesOfRepo} queue={knownQueue} />
            </>
          )}
        </nav>
        <div className="ml-auto">
          <PersonMenu me={me} onSignOut={startSignOut} />
        </div>
      </TopBar>
      {hasSignOutFailed && (
        <p
          className="border-b border-destructive-border bg-destructive-muted px-page py-2.5 text-base font-medium text-destructive"
          role="alert"
        >
          The sign-out failed. Reload the page, then try again.
        </p>
      )}
      {children}
    </>
  );
}

function PickerLabel({ text, isPlaceholder = false }: { text: string; isPlaceholder?: boolean }) {
  return (
    <>
      <span className={cn("truncate", isPlaceholder && "text-muted-foreground")}>{text}</span>
      <UpDownIcon />
    </>
  );
}

function OrgPicker({ me, org }: { me: Me; org: string }) {
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

function RepoPicker({ org, queues, repo }: { org: string; queues: Queue[]; repo: string | undefined }) {
  const repos = [...new Set(queues.map((candidate) => candidate.repo))];

  return (
    <Menu
      label="Repository"
      trigger={<PickerLabel text={repo ?? "All repositories"} isPlaceholder={!repo} />}
      triggerClassName={PICKER}
      align="left"
    >
      <Link className={menuItemVariants({ isCurrent: !repo })} to="/orgs/$org" params={{ org }}>
        All repositories<MenuHint>{queues.length}</MenuHint>
      </Link>
      <MenuSeparator />
      {repos.map((name) => (
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

/** A repository of the picker: its only queue, else the page that lists its queues. */
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
      <MenuHint>{queues.length}</MenuHint>
    </>
  );
  const [onlyQueue] = queues;

  if (onlyQueue && queues.length === 1) {
    return (
      <Link
        className={className}
        to="/orgs/$org/repos/$repo/queues/$queue"
        params={{ org, repo, queue: onlyQueue.name }}
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
  return queue.state === "running" ? "bg-success" : "bg-warning";
}

function QueuePicker({
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
          key={candidate.name}
          className={menuItemVariants({ isCurrent: candidate.name === queue })}
          to="/orgs/$org/repos/$repo/queues/$queue"
          params={{ org, repo, queue: candidate.name }}
        >
          <span className={cn("size-1.75 flex-none rounded-full", toneOf(candidate))} />
          <b className="font-semibold">{candidate.name}</b>
          <MenuHint>
            {candidate.config_error ? "invalid" : `${String(inQueueCount(candidate.counts))} in the queue`}
          </MenuHint>
        </Link>
      ))}
    </Menu>
  );
}

function PersonMenu({ me: { login, name, avatar_url }, onSignOut }: { me: Me; onSignOut: () => void }) {
  return (
    <Menu
      label="Your account"
      trigger={
        <>
          <Avatar name={login} src={avatar_url} size={24} />
          <span className="max-md:hidden">{login}</span>
        </>
      }
      triggerClassName="flex cursor-pointer items-center gap-2 rounded-sm px-1.5 py-0.5 text-base font-medium text-foreground hover:bg-muted"
      align="right"
    >
      <MenuItem className="font-medium" disabled>
        Signed in as {name ? `${name} (@${login})` : `@${login}`}
      </MenuItem>
      <MenuItem className="font-medium" onClick={toggleTheme}>
        Light / dark<MenuHint>theme</MenuHint>
      </MenuItem>
      <MenuSeparator />
      <MenuItem className="font-medium" onClick={onSignOut}>
        Sign out
      </MenuItem>
    </Menu>
  );
}
