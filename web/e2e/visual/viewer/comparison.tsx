import { type CSSProperties, type ReactNode, type RefObject, useEffect, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import type { ViewDifference } from "./data";

export type Mode = "side" | "split" | "blend" | "changes";

/** How much of the capture to show: around the change, the whole width, or its pixels as they are. */
export type Size = "change" | "fit" | "actual";

export type Side = "before" | "after";

export interface ImagePaths {
  before: string;
  after: string;
  changes: string;
}

interface ImageSize {
  width: number;
  height: number;
}

interface Crop extends ImageSize {
  x: number;
  y: number;
  scale: number;
}

// Around the changed pixels, so that the change keeps what it sits in; then the largest zoom of a small change.
const MARGIN = 96;
const MAX_ZOOM = 4;
const GAP = 16;

// Below the top of the comparison: its legend, and the padding of the card and of the page.
const BELOW = 80;

/** A key for the viewer, unless the person types in a field; a slider keeps only the arrows that move it. */
export function isForViewer(event: KeyboardEvent): boolean {
  const { target } = event;

  if (!(target instanceof HTMLInputElement)) return true;
  return target.type === "range" && !["ArrowLeft", "ArrowRight"].includes(event.key);
}

/** The room that the comparison has: the width of its column, and the height of the window below its top. */
function useRoom(ref: RefObject<HTMLElement | null>): ImageSize {
  const [room, setRoom] = useState<ImageSize>({ width: 0, height: 0 });

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const measure = () => {
      const { top, width } = element.getBoundingClientRect();
      setRoom({ width, height: window.innerHeight - top - BELOW });
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [ref]);

  return room;
}

/** The size of the image at `src`, once it loaded; `null` for no image. */
function useImageSize(src: string | null): ImageSize | null {
  const [size, setSize] = useState<{ src: string } & ImageSize>();

  useEffect(() => {
    if (!src) return;

    const image = new Image();
    image.src = src;
    image
      .decode()
      .then(() => {
        setSize({ src, width: image.naturalWidth, height: image.naturalHeight });
      })
      .catch(() => {
        setSize({ src, width: 0, height: 0 });
      });
  }, [src]);

  return size?.src === src ? size : null;
}

function cropOf(view: ViewDifference, image: ImageSize, size: Size, room: ImageSize): Crop {
  if (size === "actual") {
    return { x: 0, y: 0, ...image, scale: 1 };
  }
  if (size === "fit" || !view.box) {
    return { x: 0, y: 0, ...image, scale: Math.min(1, room.width / image.width) };
  }

  const x = Math.max(0, view.box.left - MARGIN);
  const y = Math.max(0, view.box.top - MARGIN);
  const width = Math.min(image.width, view.box.right + 1 + MARGIN) - x;
  const height = Math.min(image.height, view.box.bottom + 1 + MARGIN) - y;
  return { x, y, width, height, scale: Math.min(MAX_ZOOM, room.width / width, room.height / height) };
}

interface Layer {
  src: string;
  alt: string;
  style?: CSSProperties;
}

/** The part of the capture that `crop` names, at its scale; the layers sit on top of each other. */
function Frame({ crop, layers, children }: { crop: Crop; layers: Layer[]; children?: ReactNode }) {
  return (
    <div
      className="relative flex-none overflow-hidden rounded-sm border border-border bg-card"
      style={{ width: crop.width * crop.scale, height: crop.height * crop.scale }}
    >
      {layers.map(({ src, alt, style }) => (
        <img
          key={src}
          src={src}
          alt={alt}
          draggable={false}
          className="absolute max-w-none select-none"
          style={{
            // Each capture keeps its own size: the two of a view differ when the page grew or shrank.
            transform: `scale(${String(crop.scale)})`,
            transformOrigin: "0 0",
            left: -crop.x * crop.scale,
            top: -crop.y * crop.scale,
            // A zoomed capture shows its pixels, not a blur of them.
            imageRendering: crop.scale > 1 ? "pixelated" : "auto",
            ...style,
          }}
        />
      ))}
      {children}
    </div>
  );
}

function Legend({ children }: { children: ReactNode }) {
  return (
    <div className="mb-2 flex min-h-7 items-center gap-4 text-base text-secondary-foreground">{children}</div>
  );
}

const BEFORE = <Badge tone="neutral">Before</Badge>;
const AFTER = <Badge tone="info">After</Badge>;

function Split({ crop, paths }: { crop: Crop; paths: ImagePaths }) {
  const [split, setSplit] = useState(50);

  return (
    <div>
      <Legend>
        {BEFORE}
        <span className="text-muted-foreground">Drag the divider to compare</span>
        {AFTER}
      </Legend>
      <Frame
        crop={crop}

        layers={[
          { src: paths.before, alt: "Before" },
          { src: paths.after, alt: "After", style: { clipPath: `inset(0 0 0 ${String(split)}%)` } },
        ]}
      >
        <div
          className="pointer-events-none absolute inset-y-0 -ml-px w-0.5 bg-primary"
          style={{ left: `${String(split)}%` }}
        >
          <span className="absolute top-1/2 left-1/2 grid size-6 -translate-1/2 place-items-center rounded-full bg-primary text-sm text-primary-foreground shadow-lg">
            ↔
          </span>
        </div>
        <input
          type="range"
          min="0"
          max="100"
          step="0.1"
          value={split}
          onChange={(event) => {
            setSplit(Number(event.target.value));
          }}
          aria-label="Move the divider between before and after"
          aria-valuetext={`${String(Math.round(split))}% before, ${String(Math.round(100 - split))}% after`}
          className="absolute inset-0 size-full cursor-ew-resize opacity-0"
        />
      </Frame>
    </div>
  );
}

function Blend({ crop, paths }: { crop: Crop; paths: ImagePaths }) {
  const [blend, setBlend] = useState(50);

  useEffect(() => {
    const flip = (event: KeyboardEvent) => {
      if (event.key !== " " || !isForViewer(event)) return;

      event.preventDefault();
      setBlend((value) => (value < 50 ? 100 : 0));
    };
    window.addEventListener("keydown", flip);
    return () => {
      window.removeEventListener("keydown", flip);
    };
  }, []);

  return (
    <div>
      <Legend>
        {BEFORE}
        <input
          type="range"
          min="0"
          max="100"
          value={blend}
          onChange={(event) => {
            setBlend(Number(event.target.value));
          }}
          aria-label="Show the after image"
          className="w-60 accent-primary"
        />
        {AFTER}
        <output className="w-10 text-muted-foreground tabular-nums">{blend}%</output>
        <span className="text-muted-foreground">Space flips</span>
      </Legend>
      <Frame
        crop={crop}

        layers={[
          { src: paths.before, alt: "Before" },
          { src: paths.after, alt: "After", style: { opacity: blend / 100 } },
        ]}
      />
    </div>
  );
}

function Single({ legend, crop, layer }: { legend: ReactNode; crop: Crop; layer: Layer }) {
  return (
    <div>
      <Legend>{legend}</Legend>
      <Frame crop={crop} layers={[layer]} />
    </div>
  );
}

function SideBySide({ crop, paths }: { crop: Crop; paths: ImagePaths }) {
  return (
    <div className="flex items-start" style={{ gap: GAP }}>
      <Single legend={BEFORE} crop={crop} layer={{ src: paths.before, alt: "Before" }} />
      <Single legend={AFTER} crop={crop} layer={{ src: paths.after, alt: "After" }} />
    </div>
  );
}

/** The view before and after, as `mode` and `size` say; `side` shows one of them alone, at full width. */
export function Comparison({
  view,
  paths,
  mode,
  size,
  side,
}: {
  view: ViewDifference;
  paths: ImagePaths;
  mode: Mode;
  size: Size;
  side: Side | null;
}) {
  const container = useRef<HTMLDivElement>(null);
  const room = useRoom(container);
  const before = useImageSize(view.status === "added" ? null : paths.before);
  const after = useImageSize(view.status === "removed" ? null : paths.after);
  const isLoaded = (view.status === "added" || before) && (view.status === "removed" || after);
  // The frame holds the larger of the two captures, so that what only one of them has still shows.
  const image = {
    width: Math.max(before?.width ?? 0, after?.width ?? 0),
    height: Math.max(before?.height ?? 0, after?.height ?? 0),
  };
  const isPair = view.status === "changed" && side === null;
  const columns = isPair && mode === "side" ? 2 : 1;
  const column = { width: (room.width - GAP * (columns - 1)) / columns, height: room.height };

  return (
    <div ref={container} className="min-w-0 overflow-auto">
      {isLoaded && room.width > 0 && (
        <Shown {...{ view, paths, mode, side, columns }} crop={cropOf(view, image, size, column)} />
      )}
    </div>
  );
}

function Shown({
  view,
  paths,
  mode,
  side,
  crop,
  columns,
}: {
  view: ViewDifference;
  paths: ImagePaths;
  mode: Mode;
  side: Side | null;
  crop: Crop;
  columns: number;
}) {
  if (view.status === "removed") {
    return <Single legend={BEFORE} crop={crop} layer={{ src: paths.before, alt: "Before" }} />;
  }
  if (view.status !== "changed") {
    const legend = view.status === "added" ? AFTER : <Badge>Unchanged</Badge>;
    return <Single legend={legend} crop={crop} layer={{ src: paths.after, alt: "After" }} />;
  }
  if (side) {
    const legend = (
      <>
        {side === "before" ? BEFORE : AFTER}
        <span className="text-muted-foreground">Esc to compare</span>
      </>
    );
    return <Single legend={legend} crop={crop} layer={{ src: paths[side], alt: side }} />;
  }
  if (columns === 2) {
    return <SideBySide crop={crop} paths={paths} />;
  }
  if (mode === "split") {
    return <Split crop={crop} paths={paths} />;
  }
  if (mode === "blend") {
    return <Blend crop={crop} paths={paths} />;
  }

  const legend = (
    <>
      <Badge tone="destructive">Changed pixels</Badge>
      <span className="text-muted-foreground">over the after image, dimmed</span>
    </>
  );
  return <Single legend={legend} crop={crop} layer={{ src: paths.changes, alt: "Changed pixels" }} />;
}
