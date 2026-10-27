// What an agent reads on the site, against the build of `next build`: the
// index, the Markdown of each page, the content negotiation and the files
// that the build copies from the repository.
import assert from "node:assert/strict";
import { type ChildProcess, spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

const port = 4399;
const origin = `http://localhost:${String(port)}`;
const productionOrigin = "https://cihub.io";
const repository = join(import.meta.dirname, "..", "..", "..");

let server: ChildProcess;

before(async () => {
  const site = join(import.meta.dirname, "..");
  // Node runs Next.js itself: a kill of a pnpm wrapper would leave the server running.
  server = spawn(
    process.execPath,
    [join(site, "node_modules", "next", "dist", "bin", "next"), "start", "--port", String(port)],
    {
      cwd: site,
      stdio: "ignore",
    },
  );
  await waitForServer();
});

after(() => {
  server.kill();
});

async function waitForServer() {
  for (let attempt = 0; attempt < 100; attempt++) {
    try {
      await fetch(origin);
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw new Error(`the site did not start on ${origin}`);
}

function local(url: string) {
  return url.replace(productionOrigin, origin);
}

/** The pages that llms.txt lists, by their Markdown URL on cihub.io. */
async function listedPages() {
  const index = await (await fetch(`${origin}/llms.txt`)).text();
  const urls = [...index.matchAll(/\]\((https:\/\/cihub\.io\/docs[^)]*\.md)\)/g)].map(
    (match) => match[1] ?? "",
  );

  assert.ok(urls.length > 0, "llms.txt lists no page");
  return urls;
}

describe("llms.txt", () => {
  test("is the index of the site, in Markdown", async () => {
    const response = await fetch(`${origin}/llms.txt`);
    const body = await response.text();

    assert.equal(response.status, 200);
    assert.match(response.headers.get("content-type") ?? "", /^text\/markdown/);
    assert.match(body, /^# CIHub\n\n> /);
  });

  test("links only to URLs that answer", async () => {
    const index = await (await fetch(`${origin}/llms.txt`)).text();
    const urls = [...index.matchAll(/\]\((https:\/\/cihub\.io[^)]*)\)/g)].map((match) => match[1] ?? "");

    for (const url of urls) {
      assert.equal((await fetch(local(url))).status, 200, url);
    }
  });

  test("is the Markdown of the home page", async () => {
    const response = await fetch(origin, { headers: { Accept: "text/markdown" } });

    assert.match(await response.text(), /^# CIHub\n/);
    assert.equal(response.headers.get("vary")?.includes("Accept"), true);
  });
});

describe("a page of the documentation", () => {
  test("is Markdown at its URL with .md", async () => {
    for (const url of await listedPages()) {
      const response = await fetch(local(url));
      const body = await response.text();
      const prose = body.replace(/```[\s\S]*?```/g, "");

      assert.match(response.headers.get("content-type") ?? "", /^text\/markdown/, url);
      assert.match(body, /^---\ntitle: /, url);
      assert.equal(prose.match(/^# /gm)?.length, 1, `${url} has one H1`);
    }
  });

  test("is Markdown at its URL for a request that prefers it", async () => {
    for (const url of await listedPages()) {
      const page = local(url).replace(/\.md$/, "");
      const response = await fetch(page, { headers: { Accept: "text/markdown" } });

      assert.match(response.headers.get("content-type") ?? "", /^text\/markdown/, page);
      assert.equal(response.headers.get("vary")?.includes("Accept"), true, page);
      assert.equal(await response.text(), await (await fetch(local(url))).text(), page);
    }
  });

  test("is HTML with one H1, its canonical URL and a link to its Markdown", async () => {
    for (const url of await listedPages()) {
      const page = url.replace(/\.md$/, "");
      const html = await (await fetch(local(page))).text();

      assert.equal(html.match(/<h1[\s>]/g)?.length, 1, `${page} has one H1`);
      assert.ok(html.includes(`<link rel="canonical" href="${page}"/>`), `${page} has its canonical URL`);
      assert.ok(html.includes(`<link rel="alternate" type="text/markdown" href="${url}"/>`), page);
    }
  });

  test("that does not exist lists the pages that do", async () => {
    const response = await fetch(`${origin}/docs/quick-start.md`);
    const body = await response.text();

    assert.equal(response.status, 404);
    assert.match(body, /^# Page not found/);
    assert.ok(body.includes("/docs/install.md"));
  });
});

describe("the machine-readable files", () => {
  test("the configuration schema is the one of the repository", async () => {
    const response = await fetch(`${origin}/schema/config/v1.json`);
    const published = await response.text();
    const schema = await readFile(join(repository, "schema", "config", "v1.json"), "utf8");

    assert.equal(response.headers.get("content-type"), "application/schema+json");
    assert.equal(response.headers.get("access-control-allow-origin"), "*");
    assert.equal(published, schema);
    assert.equal((JSON.parse(schema) as { $id: string }).$id, `${productionOrigin}/schema/config/v1.json`);
  });

  test("the OpenAPI document names no server on cihub.io", async () => {
    const response = await fetch(`${origin}/openapi/v1.json`);
    const document = (await response.json()) as { servers: { url: string }[] };

    assert.equal(response.headers.get("access-control-allow-origin"), "*");
    assert.ok(document.servers.length > 0);
    assert.ok(document.servers.every((server) => !server.url.startsWith("/")));
  });
});
