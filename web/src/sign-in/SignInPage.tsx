import { useState } from "react";

import type { Me } from "../client/types.gen";
import { signInAddress, signOut } from "../session";
import styles from "./SignIn.module.css";
import { Mark } from "./Mark";
import { Timeline } from "./Timeline";
import { ArrowIcon, CheckIcon, ErrorIcon } from "./icons";

interface Props {
  /** The user of the session; a user without an organization must install the App. */
  me: Me | null;
  hasFailed: boolean;
  /** The page to open once signed in. */
  next: string;
}

export function SignInPage({ me, hasFailed, next }: Props) {
  return (
    <div className={styles.split}>
      <section className={styles.hero}>
        <div className={styles.brand}>
          <Mark className={styles.mark} />
          CIHub
        </div>
        <h2>Never break main again.</h2>
        <p>
          Every pull request is tested on top of everything that lands before it, and merges only if the
          result still passes.
        </p>
        <Timeline className={styles.timeline} />
        <ul>
          <li>
            <CheckIcon />
            <span>
              What is tested is exactly what lands on <code>main</code>.
            </span>
          </li>
          <li>
            <CheckIcon />
            <span>Your CI, unchanged: CIHub waits for its required checks.</span>
          </li>
          <li>
            <CheckIcon />
            <span>Every pull request says why it waits.</span>
          </li>
        </ul>
      </section>
      <section className={styles.formSide}>
        <div className={styles.form}>
          <div className={styles.brand}>
            <Mark className={styles.mark} />
            CIHub
          </div>
          {me ? (
            <NoOrganization installUrl={me.app_install_url ?? null} />
          ) : (
            <SignIn hasFailed={hasFailed} next={next} />
          )}
          <div className={styles.hr} />
          <p className={`${styles.note} ${styles.flush}`}>
            Scripts and the <code>cihub</code> CLI sign in with an API token instead.
          </p>
        </div>
      </section>
    </div>
  );
}

function SignIn({ hasFailed, next }: { hasFailed: boolean; next: string }) {
  const [isLeaving, setIsLeaving] = useState(false);
  return (
    <>
      <h1>Sign in</h1>
      <p className={styles.lead}>See and drive the merge queues of your organizations.</p>
      {hasFailed && (
        <div className={styles.alert} role="alert">
          <ErrorIcon />
          <span>The sign-in failed. Try again.</span>
        </div>
      )}
      <a
        className={`${styles.button} ${styles.primary}`}
        href={signInAddress(next)}
        aria-disabled={isLeaving}
        onClick={() => {
          setIsLeaving(true);
        }}
      >
        {isLeaving ? (
          <>
            <span className={styles.spinner} />
            Redirecting to GitHub
          </>
        ) : (
          <>
            Continue with GitHub <ArrowIcon />
          </>
        )}
      </a>
      <p className={styles.note}>
        CIHub asks GitHub who you are and which organizations of its App you can reach. It does not keep your
        GitHub token. A session lasts 14 days.
      </p>
    </>
  );
}

/** A user whose account is in no organization of the App: the way out is to install it. */
function NoOrganization({ installUrl }: { installUrl: string | null }) {
  const [hasSignOutFailed, setHasSignOutFailed] = useState(false);
  return (
    <>
      <h1>No organization yet</h1>
      <p className={styles.lead}>
        Your GitHub account is not a member of an organization where the CIHub App is installed.
      </p>
      {installUrl && (
        <a className={`${styles.button} ${styles.primary}`} href={installUrl}>
          Install the App on GitHub <ArrowIcon />
        </a>
      )}
      <p className={styles.note}>
        Only an owner of the organization can install it. If you are not one, ask an owner, then sign in
        again.
      </p>
      <button
        type="button"
        className={`${styles.button} ${styles.second}`}
        onClick={() => {
          void signOut().then((hasSignedOut) => {
            if (hasSignedOut) {
              window.location.assign(signInAddress("/", true));
            } else {
              setHasSignOutFailed(true);
            }
          });
        }}
      >
        Sign in with another account
      </button>
      {hasSignOutFailed && (
        <div className={`${styles.alert} ${styles.below}`} role="alert">
          <ErrorIcon />
          <span>The sign-out failed. Reload the page, then try again.</span>
        </div>
      )}
    </>
  );
}
