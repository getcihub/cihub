import { Link, useNavigate } from "@tanstack/react-router";
import { type ReactNode, useCallback, useRef } from "react";

import { useDialog } from "../ui/dialog";
import styles from "./PrDrawer.module.css";
import type { QueuePath } from "./reads";

interface Props {
  path: QueuePath;
  title: ReactNode;
  /** Above the title. */
  meta?: ReactNode;
  /** Under the title, in the head. */
  head?: ReactNode;
  children?: ReactNode;
}

/**
 * The frame of a drawer over the page of a queue: Close, Escape and a click beside it go back to the page, with
 * its tab; the focus goes to the title, and comes back to where it was.
 */
export function DrawerFrame({ path, title, meta, head, children }: Props) {
  const navigate = useNavigate();
  const drawer = useRef<HTMLElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const close = useCallback(() => {
    void navigate({ to: "/orgs/$org/repos/$repo/queues/$queue", params: path, search: (kept) => kept });
  }, [navigate, path]);
  useDialog(drawer, close, { first: heading });

  return (
    <>
      <div className={styles.scrim} onClick={close} aria-hidden="true" />
      <aside
        ref={drawer}
        className={styles.drawer}
        role="dialog"
        aria-modal="true"
        aria-labelledby="pr-title"
      >
        <header>
          <Link
            className={styles.close}
            to="/orgs/$org/repos/$repo/queues/$queue"
            params={path}
            search={(kept) => kept}
          >
            Close
          </Link>
          {meta && <div className={styles.meta}>{meta}</div>}
          <h2 id="pr-title" ref={heading} tabIndex={-1}>
            {title}
          </h2>
          {head}
        </header>
        {children}
      </aside>
    </>
  );
}
