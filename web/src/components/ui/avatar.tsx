import { useState } from "react";

// The colors of the avatars of the mockups: the color of a person, not a token of the theme.
const COLORS = ["#7C5CFF", "#1A7F64", "#D9480F", "#C2255C", "#1971C2", "#5C940D", "#9C36B5", "#0B7285"];

/**
 * The image at `src` in a circle; without it, or when it does not load, the
 * first letter of `name` with a color that the name keeps.
 */
export function Avatar({ name, src, size = 20 }: { name: string; src?: string | null; size?: number }) {
  const [failedSrc, setFailedSrc] = useState<string | null>(null);

  if (src && src !== failedSrc) {
    return (
      <img
        data-slot="avatar"
        className="flex-none rounded-full"
        src={src}
        width={size}
        height={size}
        alt=""
        referrerPolicy="no-referrer"
        onError={() => {
          setFailedSrc(src);
        }}
      />
    );
  }

  let sum = 0;
  for (let index = 0; index < name.length; index++) {
    sum += name.charCodeAt(index);
  }
  const color = COLORS[sum % COLORS.length];

  return (
    <span
      data-slot="avatar"
      className="inline-grid flex-none place-items-center rounded-full text-3xs font-bold text-primary-foreground"
      style={{ width: size, height: size, background: color }}
      aria-hidden="true"
    >
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
