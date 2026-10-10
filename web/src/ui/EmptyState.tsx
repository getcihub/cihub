import type { ReactNode } from "react";

import styles from "./ui.module.css";

interface Props {
  illustration: ReactNode;
  title: string;
  /** `h1` when the empty state is the whole page. */
  heading?: "h1" | "h3";
  children: ReactNode;
  actions?: ReactNode;
}

/** A page or a card with nothing to show: what happened, and what to do. */
export function EmptyState({ illustration, title, heading: Heading = "h3", children, actions }: Props) {
  return (
    <div className={styles.empty}>
      {illustration}
      <Heading className={styles.emptyTitle}>{title}</Heading>
      <p>{children}</p>
      {actions && <div className={styles.actions}>{actions}</div>}
    </div>
  );
}
