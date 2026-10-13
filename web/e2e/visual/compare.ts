// Compares the captures of two builds, pixel by pixel: `node e2e/visual/compare.ts BASE HEAD OUT [LABEL]`. For each
// view that differs, OUT/changes gets the head dimmed with the changed pixels in red; OUT/report.md lists the views,
// and OUT/index.html shows each view before and after, with the viewer that shots.sh builds into OUT/viewer.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

import { chromium } from "@playwright/test";

import type { Box, ViewerData } from "./viewer/data";

interface Difference {
  name: string;
  /** The number of pixels that changed; `null` when the size of the view changed. */
  pixels: number | null;
  region: string;
  /** The smallest rectangle that holds the changed pixels; `null` when the size of the view changed. */
  box: Box | null;
}

const [baseDir, headDir, outDir, label = "the base"] = process.argv.slice(2);
if (!baseDir || !headDir || !outDir) {
  console.error("usage: node e2e/visual/compare.ts BASE HEAD OUT [LABEL]");
  process.exit(2);
}

const pngs = (dir: string): string[] =>
  existsSync(dir) ? readdirSync(dir).filter((name) => name.endsWith(".png")) : [];
const dataUrl = (file: string): string => `data:image/png;base64,${readFileSync(file).toString("base64")}`;

// The page compares the two images on canvases, and draws the head dimmed with the changed pixels in red.
const PAGE = `<body style="margin:0;background:#fff"><img id="base" hidden><img id="head" hidden>
<canvas id="changes" style="display:block"></canvas></body>`;

function compareInPage(): Omit<Difference, "name"> {
  const base = document.getElementById("base") as HTMLImageElement;
  const head = document.getElementById("head") as HTMLImageElement;
  const changes = document.getElementById("changes") as HTMLCanvasElement;
  const { naturalWidth: width, naturalHeight: height } = head;

  if (base.naturalWidth !== width || base.naturalHeight !== height) {
    return {
      pixels: null,
      box: null,
      region: `size ${String(base.naturalWidth)}×${String(base.naturalHeight)} → ${String(width)}×${String(height)}`,
    };
  }

  const pixelsOf = (image: HTMLImageElement): Uint8ClampedArray => {
    const canvas = new OffscreenCanvas(width, height);
    const context = canvas.getContext("2d");
    context?.drawImage(image, 0, 0);
    return context?.getImageData(0, 0, width, height).data ?? new Uint8ClampedArray();
  };
  const [before, after] = [pixelsOf(base), pixelsOf(head)];
  changes.width = width;
  changes.height = height;
  const context = changes.getContext("2d");
  const out = new ImageData(width, height);
  let count = 0;
  let [left, top, right, bottom] = [width, height, -1, -1];

  for (let index = 0; index < after.length; index += 4) {
    const isChanged =
      before[index] !== after[index] ||
      before[index + 1] !== after[index + 1] ||
      before[index + 2] !== after[index + 2];
    const pixel = index / 4;
    const [x, y] = [pixel % width, Math.floor(pixel / width)];

    if (isChanged) {
      count += 1;
      [left, top, right, bottom] = [
        Math.min(left, x),
        Math.min(top, y),
        Math.max(right, x),
        Math.max(bottom, y),
      ];
      out.data.set([229, 72, 77, 255], index);
    } else {
      const gray = ((after[index] ?? 0) + (after[index + 1] ?? 0) + (after[index + 2] ?? 0)) / 3;
      out.data.set([gray, gray, gray, 60], index);
    }
  }
  context?.putImageData(out, 0, 0);

  return {
    pixels: count,
    box: { left, top, right, bottom },
    region: count === 0 ? "" : `x ${String(left)}–${String(right)}, y ${String(top)}–${String(bottom)}`,
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
      ({ name, pixels, region }) =>
        `| \`${name.replace(/\.png$/, "")}\` | ${pixels === null ? "–" : String(pixels)} | ${region} |`,
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
mkdirSync(join(outDir, "changes"), { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
const differences: Difference[] = [];
const unchanged: string[] = [];

for (const name of shared) {
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
  const difference = await page.evaluate(compareInPage);

  if (difference.pixels === 0) {
    unchanged.push(name);
  } else {
    differences.push({ name, ...difference });
  }
  if (difference.pixels) {
    await page.locator("#changes").screenshot({ path: join(outDir, "changes", name) });
  }
}
await browser.close();

const report = reportOf(differences, added, removed, shared.length);
writeFileSync(join(outDir, "report.md"), report);
console.log(report);

const viewerData: ViewerData = {
  label,
  before: relative(outDir, baseDir),
  after: relative(outDir, headDir),
  views: [
    ...differences.map(({ name, pixels, box }) => ({ name, status: "changed" as const, pixels, box })),
    ...added.map((name) => ({ name, status: "added" as const, pixels: null, box: null })),
    ...removed.map((name) => ({ name, status: "removed" as const, pixels: null, box: null })),
    ...unchanged.map((name) => ({ name, status: "unchanged" as const, pixels: null, box: null })),
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
