import frame from "../frame/Frame.module.css";
import { Mark } from "../ui/Mark";
import styles from "../ui/ui.module.css";

/** While a page reads its data: the top bar, empty, so that the page does not jump. */
export function PendingPage() {
  return (
    <>
      <header className={frame.topbar}>
        <span className={frame.logo}>
          <Mark className={frame.mark} />
        </span>
      </header>
      <main className={styles.page} aria-busy="true">
        <span className={styles.hidden} role="status">
          Loading
        </span>
      </main>
    </>
  );
}
