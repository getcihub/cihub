// Compares the captures of two builds, pixel by pixel: `node e2e/visual/compare.ts BASE HEAD OUT [LABEL]`. For each
// view that differs, OUT/changes gets the head dimmed with the changed pixels in red; OUT/report.md lists the views,
// and OUT/index.html shows each view before and after, with the viewer that shots.sh builds into OUT/viewer.
// OUT/before and OUT/after hold the captures of the views that differ, so that OUT is the whole comparison without
// the views that did not change: the CI uploads only OUT.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { chromium } from "@playwright/test";
import pixelmatch from "pixelmatch";

import type { Box, ViewerData } from "./viewer/data";

interface Difference {
  name: string;
  /** The number of pixels that changed, the pixels that only one capture has included. */
  pixels: number;
  region: string;
  /** The smallest rectangle that holds the changed pixels. */
  box: Box;
  /** The size before and after, when it changed: "1280×800 → 1280×923". */
  resized: string | null;
}

const [baseDir, headDir, outDir, label = "the base"] = process.argv.slice(2);
if (!baseDir || !headDir || !outDir) {
  console.error("usage: node e2e/visual/compare.ts BASE HEAD OUT [LABEL]");
  process.exit(2);
}

const pngs = (dir: string): string[] =>
  existsSync(dir) ? readdirSync(dir).filter((name) => name.endsWith(".png")) : [];
const dataUrl = (file: string): string => `data:image/png;base64,${readFileSync(file).toString("base64")}`;

// The page draws the two images on canvases of the larger of their sizes: a pixel that only one image has is
// transparent in the other one.
const PAGE = `<body style="margin:0;background:#fff"><img id="base" hidden><img id="head" hidden>
<canvas id="changes" style="display:block"></canvas></body>`;

interface Pixels {
  width: number;
  height: number;
  before: Uint8ClampedArray;
  after: Uint8ClampedArray;
  /** The size that both images cover. */
  shared: { width: number; height: number };
  /** The size before and after, when it changed. */
  resized: string | null;
}

function pixelsInPage(): Pixels {
  const base = document.getElementById("base") as HTMLImageElement;
  const head = document.getElementById("head") as HTMLImageElement;
  const width = Math.max(base.naturalWidth, head.naturalWidth);
  const height = Math.max(base.naturalHeight, head.naturalHeight);
  const sizeOf = (image: HTMLImageElement): string =>
    `${String(image.naturalWidth)}×${String(image.naturalHeight)}`;
  const pixelsOf = (image: HTMLImageElement): Uint8ClampedArray => {
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d");
    context?.drawImage(image, 0, 0);
    return context?.getImageData(0, 0, width, height).data ?? new Uint8ClampedArray();
  };

  return {
    width,
    height,
    before: pixelsOf(base),
    after: pixelsOf(head),
    shared: {
      width: Math.min(base.naturalWidth, head.naturalWidth),
      height: Math.min(base.naturalHeight, head.naturalHeight),
    },
    resized: sizeOf(base) === sizeOf(head) ? null : `${sizeOf(base)} → ${sizeOf(head)}`,
  };
}

function drawInPage({
  width,
  height,
  changes,
}: {
  width: number;
  height: number;
  changes: Uint8ClampedArray;
}): void {
  const canvas = document.getElementById("changes") as HTMLCanvasElement;
  canvas.width = width;
  canvas.height = height;
  canvas.getContext("2d")?.putImageData(new ImageData(new Uint8ClampedArray(changes), width, height), 0, 0);
}

// The head dimmed, with the changed pixels in red. pixelmatch ignores the pixels of an anti-aliased edge and the
// differences that the eye does not see: the runner of the CI draws the edges of the same build with a few
// different pixels from one capture to the next.
function compare({ width, height, before, after, shared, resized }: Pixels): {
  difference: Omit<Difference, "name">;
  changes: Uint8ClampedArray;
} {
  const changed = new Uint8ClampedArray(before.length);
  pixelmatch(before, after, changed, width, height, { diffMask: true });
  const changes = new Uint8ClampedArray(before.length);
  let count = 0;
  let [left, top, right, bottom] = [width, height, -1, -1];

  for (let index = 0; index < after.length; index += 4) {
    const pixel = index / 4;
    const [x, y] = [pixel % width, Math.floor(pixel / width)];
    const isChanged = changed[index + 3] !== 0 || x >= shared.width || y >= shared.height;

    if (isChanged) {
      count += 1;
      [left, top, right, bottom] = [
        Math.min(left, x),
        Math.min(top, y),
        Math.max(right, x),
        Math.max(bottom, y),
      ];
      changes.set([229, 72, 77, 255], index);
    } else {
      const gray = ((after[index] ?? 0) + (after[index + 1] ?? 0) + (after[index + 2] ?? 0)) / 3;
      changes.set([gray, gray, gray, 60], index);
    }
  }

  const area = `x ${String(left)}–${String(right)}, y ${String(top)}–${String(bottom)}`;
  return {
    difference: {
      pixels: count,
      box: { left, top, right, bottom },
      region: count === 0 ? "" : resized ? `${area}; size ${resized}` : area,
      resized,
    },
    changes,
  };
}

const views = (count: number): string => `${String(count)} ${count === 1 ? "view" : "views"}`;
const viewList = (names: string[]): string =>
  names.map((name) => `\`${name.replace(/\.png$/, "")}\``).join(", ");

function reportOf(differences: Difference[], added: string[], removed: string[], compared: number): string {
  const sections = ["## Visual changes"];

  if (compared > 0) {
    sections.push(
      differences.length === 0
        ? `The ${views(compared)} are the same as in ${label}.`
        : `${String(differences.length)} of ${views(compared)} differ from ${label}.`,
    );
  }
  if (differences.length > 0) {
    const rows = differences.map(
      ({ name, pixels, region }) => `| \`${name.replace(/\.png$/, "")}\` | ${String(pixels)} | ${region} |`,
    );
    sections.push(["| View | Changed pixels | Region |", "| --- | ---: | --- |", ...rows].join("\n"));
  }
  if (added.length > 0) {
    sections.push(`New in the head, ${views(added.length)} that ${label} does not show: ${viewList(added)}.`);
  }
  if (removed.length > 0) {
    sections.push(`Gone from the head, ${views(removed.length)} of ${label}: ${viewList(removed)}.`);
  }
  return `${sections.join("\n\n")}\n`;
}

const baseNames = pngs(baseDir);
const headNames = pngs(headDir);
const shared = headNames.filter((name) => baseNames.includes(name)).sort();
const added = headNames.filter((name) => !baseNames.includes(name)).sort();
const removed = baseNames.filter((name) => !headNames.includes(name)).sort();
for (const folder of ["changes", "before", "after"]) {
  mkdirSync(join(outDir, folder), { recursive: true });
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
const differences: Difference[] = [];

for (const name of shared) {
  if (readFileSync(join(baseDir, name)).equals(readFileSync(join(headDir, name)))) {
    continue;
  }

  await page.setContent(PAGE);
  await page.evaluate(
    async ([base, head]) => {
      const load = (id: string, src: string) =>
        new Promise((done) => {
          const image = document.getElementById(id) as HTMLImageElement;
          image.onload = done;
          image.src = src;
        });
      await Promise.all([load("base", base ?? ""), load("head", head ?? "")]);
    },
    [dataUrl(join(baseDir, name)), dataUrl(join(headDir, name))],
  );
  const pixels = await page.evaluate(pixelsInPage);
  const { difference, changes } = compare(pixels);

  if (difference.pixels) {
    differences.push({ name, ...difference });
    await page.evaluate(drawInPage, { width: pixels.width, height: pixels.height, changes });
    await page.locator("#changes").screenshot({ path: join(outDir, "changes", name) });
  }
}
await browser.close();

for (const { name } of differences) {
  copyFileSync(join(baseDir, name), join(outDir, "before", name));
  copyFileSync(join(headDir, name), join(outDir, "after", name));
}
for (const name of added) {
  copyFileSync(join(headDir, name), join(outDir, "after", name));
}
for (const name of removed) {
  copyFileSync(join(baseDir, name), join(outDir, "before", name));
}

const report = reportOf(differences, added, removed, shared.length);
writeFileSync(join(outDir, "report.md"), report);
console.log(report);

const viewerData: ViewerData = {
  label,
  compared: shared.length,
  views: [
    ...differences.map(({ name, pixels, box, resized }) => ({
      name,
      status: "changed" as const,
      pixels,
      box,
      resized,
    })),
    ...added.map((name) => ({ name, status: "added" as const, pixels: null, box: null, resized: null })),
    ...removed.map((name) => ({ name, status: "removed" as const, pixels: null, box: null, resized: null })),
  ],
};
// The theme is set before the first paint, as the dashboard does (index.html).
const viewer = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Visual changes</title>
<script>
document.documentElement.dataset.theme = localStorage.getItem("theme") ??
  (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
</script>
<link rel="stylesheet" href="viewer/viewer.css">
</head>
<body>
<div id="root"></div>
<script id="viewer-data" type="application/json">${JSON.stringify(viewerData).replaceAll("<", "\\u003c")}</script>
<script src="viewer/viewer.js"></script>
</body>
</html>
`;
writeFileSync(join(outDir, "index.html"), viewer);
