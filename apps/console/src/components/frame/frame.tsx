import { Link } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";

import type { Queue } from "@/client/types.gen";
import { Mark } from "@cihub/icons/brand/mark";
import { CheckIcon } from "@cihub/icons/check";
import { ExternalIcon } from "@cihub/icons/external";
import { UpDownIcon } from "@cihub/icons/up-down";
import { Avatar } from "@cihub/ui/components/avatar";
import {
  Menu,
  MenuHint,
  MenuItem,
  menuItemVariants,
  MenuLabel,
  MenuSeparator,
  MenuSub,
} from "@cihub/ui/components/menu";
import { cn } from "@cihub/ui/lib/cn";
import { inQueueCount } from "@/domain/queue/counts";
import { type Me, membershipOf } from "@/domain/session/session";
import { useSignOut } from "@/hooks/use-sign-out";
import { type Theme, chooseTheme, chosenTheme } from "@/lib/preferences";

interface Props {
  me: Me;
  /** The organization of the URL. */
  org: string;
  /** Its queues, by repository then target branch; empty once access is lost. */
  queues: Queue[];
  /** The names of its repositories, those without a queue too; none while they load. */
  repos?: string[];
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
export function Frame({ me, org, queues, repos = [], repo, queue, children }: Props) {
  const { startSignOut, hasSignOutFailed } = useSignOut();
  const names = [...new Set([...repos, ...queues.map((candidate) => candidate.repo)])].sort();
  // The frame names only a repository and a queue that exist: a wrong address shows "not found" under it.
  const knownRepo = repo !== undefined && names.includes(repo) ? repo : undefined;
  const queuesOfRepo = queues.filter((candidate) => candidate.repo === knownRepo);
  const knownQueue = queuesOfRepo.some((candidate) => candidate.target_branch === queue) ? queue : undefined;

  return (
    <>
      <TopBar>
        <TopBarLogo org={org} />
        <nav className="flex min-w-0 items-center gap-0.5 font-medium" aria-label="Where you are">
          <OrgPicker me={me} org={org} />
          {names.length > 0 && (
            <>
              <span className="text-waiting">/</span>
              <RepoPicker org={org} names={names} queues={queues} repo={knownRepo} />
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

function RepoPicker({
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

const REPOSITORY_URL = "https://github.com/getcihub/cihub";

/** A page outside the dashboard, in a new tab. */
function ExternalLink({ href, label, hint }: { href: string; label: string; hint?: string }) {
  return (
    <a className={menuItemVariants()} href={href} target="_blank" rel="noreferrer">
      {label}
      {hint && <MenuHint>{hint}</MenuHint>}
      <ExternalIcon className={cn("size-3.5 flex-none text-muted-foreground", !hint && "ml-auto")} />
    </a>
  );
}

const THEMES: { theme: Theme; label: string }[] = [
  { theme: "light", label: "Light" },
  { theme: "dark", label: "Dark" },
  { theme: "system", label: "System" },
];

function PersonMenu({
  me: { login, name, avatar_url, version },
  onSignOut,
}: {
  me: Me;
  onSignOut: () => void;
}) {
  const [theme, setTheme] = useState(chosenTheme);

  return (
    <Menu
      label="Your account"
      trigger={
        <>
          <Avatar name={login} src={avatar_url} size={24} />
          <span className="sr-only">{login}</span>
        </>
      }
      triggerClassName="flex cursor-pointer items-center rounded-full p-0.5 text-foreground hover:bg-muted"
      align="right"
    >
      <MenuLabel>
        <div className="font-medium">{name ?? login}</div>
        {name && <div className="text-sm text-muted-foreground">@{login}</div>}
      </MenuLabel>
      <MenuSeparator />
      <MenuSub label="Appearance">
        {THEMES.map((choice) => (
          <MenuItem
            key={choice.theme}
            aria-pressed={choice.theme === theme}
            onClick={() => {
              chooseTheme(choice.theme);
              setTheme(choice.theme);
            }}
          >
            {choice.label}
            {choice.theme === theme && <CheckIcon className="ml-auto size-4 flex-none" />}
          </MenuItem>
        ))}
      </MenuSub>
      <MenuSeparator />
      <ExternalLink href={`${REPOSITORY_URL}#readme`} label="Documentation" />
      <ExternalLink href={`${REPOSITORY_URL}/releases/tag/v${version}`} label="About" hint={`v${version}`} />
      <ExternalLink href={REPOSITORY_URL} label="GitHub" />
      <MenuSeparator />
      <MenuItem onClick={onSignOut}>Sign out</MenuItem>
    </Menu>
  );
}
