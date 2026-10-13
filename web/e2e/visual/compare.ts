// Compares the captures of two builds, pixel by pixel: `node e2e/visual/compare.ts BASE HEAD OUT [LABEL]`. For each
// view that differs, OUT gets an image (base | head | the changed pixels in red), and OUT/report.md lists them.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { chromium } from "@playwright/test";

interface Difference {
  name: string;
  /** The number of pixels that changed; `null` when the size of the view changed. */
  pixels: number | null;
  region: string;
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
const PAGE = `<body style="margin:0;font:14px sans-serif;background:#fff">
<div style="display:flex;gap:16px;padding:8px;align-items:flex-start">
  <div><b>base</b><br><img id="base"></div><div><b>head</b><br><img id="head"></div>
  <div><b>changes</b><br><canvas id="changes"></canvas></div>
</div></body>`;

function compareInPage(): { pixels: number | null; region: string } {
  const base = document.getElementById("base") as HTMLImageElement;
  const head = document.getElementById("head") as HTMLImageElement;
  const changes = document.getElementById("changes") as HTMLCanvasElement;
  const { naturalWidth: width, naturalHeight: height } = head;

  if (base.naturalWidth !== width || base.naturalHeight !== height) {
    return {
      pixels: null,
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
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
const differences: Difference[] = [];

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
  const { pixels, region } = await page.evaluate(compareInPage);

  if (pixels !== 0) {
    differences.push({ name, pixels, region });
    await page.screenshot({ path: join(outDir, name), fullPage: true });
  }
}
await browser.close();

const report = reportOf(differences, added, removed, shared.length);
writeFileSync(join(outDir, "report.md"), report);
console.log(report);
