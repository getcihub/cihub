// What compare.ts gives the viewer, in the JSON of OUT/index.html.

export type ViewStatus = "changed" | "added" | "removed" | "unchanged";

export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface ViewDifference {
  /** The file of the capture: `queue-live-light-1280.png`. */
  name: string;
  status: ViewStatus;
  /** The number of pixels that changed; `null` when the view is not changed. */
  pixels: number | null;
  /** The smallest rectangle that holds the changed pixels; `null` when the view is not changed. */
  box: Box | null;
  /** The size before and after, when it changed: "1280×800 → 1280×923". */
  resized: string | null;
}

export interface ViewerData {
  /** What the head is compared with: "`origin/main` (53097b6)". */
  label: string;
  /** The folders of the captures, relative to OUT. */
  before: string;
  after: string;
  views: ViewDifference[];
}

// The name of a view ends with its theme and the width of its window (views.visual.ts).
const VARIANT = /^(.+)-(light|dark)-(\d+)$/;

/** `queue-live-light-1280.png`: the page `queue-live`, in the variant `light · 1280px`. */
export function partsOf(name: string): { page: string; variant: string } {
  const bare = name.replace(/\.png$/, "");
  const match = VARIANT.exec(bare);

  if (!match) {
    return { page: bare, variant: "" };
  }

  const [, page = bare, theme = "", width = ""] = match;
  return { page, variant: `${theme} · ${width}px` };
}
