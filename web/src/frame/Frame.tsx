import { Link } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";

import type { Me, OrgQueue } from "../client/types.gen";
import { toggleTheme } from "../preferences";
import { signOutAndLeave } from "../session";
import { Avatar } from "../ui/Avatar";
import { Mark } from "../ui/Mark";
import { UpDownIcon } from "../ui/icons";
import styles from "./Frame.module.css";
import { Menu } from "./Menu";

interface Props {
  me: Me;
  /** The organization of the URL. */
  org: string;
  /** Its queues, by repository then name; empty once access is lost. */
  queues: OrgQueue[];
  repo?: string | undefined;
  queue?: string | undefined;
  children: ReactNode;
}

/** The top bar of every page of an organization: CIHub / organization / repository / queue. */
export function Frame({ me, org, queues, repo, queue, children }: Props) {
  const [hasSignOutFailed, setHasSignOutFailed] = useState(false);
  const queuesOfRepo = queues.filter((candidate) => candidate.repo === repo);

  return (
    <>
      <header className={styles.topbar}>
        <Link className={styles.logo} to="/orgs/$org" params={{ org }} aria-label="CIHub">
          <Mark className={styles.mark} />
        </Link>
        <nav className={styles.crumbs} aria-label="Where you are">
          <OrgPicker me={me} org={org} />
          {queues.length > 0 && (
            <>
              <span className={styles.sep}>/</span>
              <RepoPicker org={org} queues={queues} repo={repo} />
            </>
          )}
          {repo && queue && queuesOfRepo.length > 0 && (
            <>
              <span className={styles.sep}>/</span>
              <QueuePicker org={org} repo={repo} queues={queuesOfRepo} queue={queue} />
            </>
          )}
        </nav>
        <div className={styles.me}>
          <PersonMenu
            login={me.login}
            onSignOutFailed={() => {
              setHasSignOutFailed(true);
            }}
          />
        </div>
      </header>
      {hasSignOutFailed && (
        <p className={styles.alert} role="alert">
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
      <span className={`${styles.label} ${isPlaceholder ? styles.placeholder : ""}`}>{text}</span>
      <UpDownIcon />
    </>
  );
}

function OrgPicker({ me, org }: { me: Me; org: string }) {
  return (
    <Menu
      label="Organization"
      trigger={<PickerLabel text={org} />}
      triggerClassName={styles.picker}
      align="left"
    >
      {me.orgs.map((membership) => (
        <Link
          key={membership.slug}
          className={`${styles.item} ${membership.slug.toLowerCase() === org.toLowerCase() ? styles.current : ""}`}
          to="/orgs/$org"
          params={{ org: membership.slug }}
        >
          <Avatar name={membership.slug} />
          {membership.slug}
          <span className={styles.hint}>{membership.role === "admin" ? "Admin" : "Member"}</span>
        </Link>
      ))}
      {me.app_install_url && (
        <>
          <div className={styles.divider} />
          <a className={styles.item} href={me.app_install_url}>
            Add an organization<span className={styles.hint}>Install the App</span>
          </a>
        </>
      )}
    </Menu>
  );
}

function RepoPicker({ org, queues, repo }: { org: string; queues: OrgQueue[]; repo: string | undefined }) {
  const repos = [...new Set(queues.map((candidate) => candidate.repo))];

  return (
    <Menu
      label="Repository"
      trigger={<PickerLabel text={repo ?? "All repositories"} isPlaceholder={!repo} />}
      triggerClassName={styles.picker}
      align="left"
    >
      <Link className={`${styles.item} ${repo ? "" : styles.current}`} to="/orgs/$org" params={{ org }}>
        All repositories<span className={styles.hint}>{queues.length}</span>
      </Link>
      <div className={styles.divider} />
      {repos.map((name) => (
        <Link
          key={name}
          className={`${styles.item} ${name === repo ? styles.current : ""}`}
          to="/orgs/$org/repos/$repo"
          params={{ org, repo: name }}
        >
          <span>
            <span className={styles.owner}>{org}/</span>
            <b>{name}</b>
          </span>
          <span className={styles.hint}>{queues.filter((candidate) => candidate.repo === name).length}</span>
        </Link>
      ))}
    </Menu>
  );
}

/** The dot of a queue: red for a file that is not valid, green when it runs, amber else. */
function toneOf({ queue }: OrgQueue): string {
  if (queue.config_error) {
    return styles.red;
  }
  return queue.state === "running" ? styles.green : styles.amber;
}

function liveCountOf({ counts }: OrgQueue): number {
  return counts.not_ready + counts.pending + counts.testing + counts.pending_failure + counts.tests_passed;
}

function QueuePicker({
  org,
  repo,
  queues,
  queue,
}: {
  org: string;
  repo: string;
  queues: OrgQueue[];
  queue: string;
}) {
  return (
    <Menu label="Queue" trigger={<PickerLabel text={queue} />} triggerClassName={styles.picker} align="left">
      {queues.map((candidate) => (
        <Link
          key={candidate.queue.name}
          className={`${styles.item} ${candidate.queue.name === queue ? styles.current : ""}`}
          to="/orgs/$org/repos/$repo/queues/$queue"
          params={{ org, repo, queue: candidate.queue.name }}
        >
          <span className={`${styles.dot} ${toneOf(candidate)}`} />
          <b>{candidate.queue.name}</b>
          <span className={styles.hint}>
            {candidate.queue.config_error ? "invalid" : `${String(liveCountOf(candidate))} in the queue`}
          </span>
        </Link>
      ))}
    </Menu>
  );
}

function PersonMenu({ login, onSignOutFailed }: { login: string; onSignOutFailed: () => void }) {
  return (
    <Menu
      label="Your account"
      trigger={
        <>
          <Avatar name={login} size={24} />
          <span className={styles.name}>{login}</span>
        </>
      }
      triggerClassName={styles.person}
      align="right"
    >
      <button type="button" className={styles.item} disabled>
        Signed in as @{login}
      </button>
      <button type="button" className={styles.item} onClick={toggleTheme}>
        Light / dark<span className={styles.hint}>theme</span>
      </button>
      <div className={styles.divider} />
      <button
        type="button"
        className={styles.item}
        onClick={() => {
          void signOutAndLeave().then((hasSignedOut) => {
            if (!hasSignedOut) {
              onSignOutFailed();
            }
          });
        }}
      >
        Sign out
      </button>
    </Menu>
  );
}
