import styles from "./ui.module.css";

// The colors of the avatars of the mockups.
const COLORS = ["#7C5CFF", "#1A7F64", "#D9480F", "#C2255C", "#1971C2", "#5C940D", "#9C36B5", "#0B7285"];

/** The first letter of `name` in a circle, with a color that the name keeps. */
export function Avatar({ name, size = 20 }: { name: string; size?: number }) {
  let sum = 0;
  for (let index = 0; index < name.length; index++) {
    sum += name.charCodeAt(index);
  }
  const color = COLORS[sum % COLORS.length];

  return (
    <span
      className={styles.avatar}
      style={{ width: size, height: size, background: color }}
      aria-hidden="true"
    >
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
