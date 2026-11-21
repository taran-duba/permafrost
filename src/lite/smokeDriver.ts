// Permafrost Lite's CI smoke-test UI driver. It only loads when Rust says this is a
// smoke run. It clicks through the real screens against the demo drive and reports
// each step back, so a broken screen or button fails CI.
import { api } from "../api";

type Step = { name: string; ok: boolean; detail?: string };

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function waitFor<T>(what: string, fn: () => T | null | undefined | false, ms = 20000): Promise<T> {
  const end = Date.now() + ms;
  for (;;) {
    const v = fn();
    if (v) return v;
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await sleep(100);
  }
}

const all = (sel: string, root: ParentNode = document) => Array.from(root.querySelectorAll<HTMLElement>(sel));
const text = (e: Element) => (e.textContent ?? "").replace(/\s+/g, " ").trim();
const button = (label: string, root: ParentNode = document) =>
  all("button", root).find((b) => text(b) === label && !(b as HTMLButtonElement).disabled);

async function click(what: string, find: () => HTMLElement | null | undefined) {
  (await waitFor(what, find)).click();
}

export async function runLiteSmokeUi(): Promise<void> {
  const steps: Step[] = [];
  const step = async (name: string, fn: () => Promise<void>) => {
    try {
      await fn();
      steps.push({ name, ok: true });
    } catch (e) {
      const page = text(document.querySelector(".lite") ?? document.body).slice(0, 400);
      steps.push({ name, ok: false, detail: `${e instanceof Error ? e.message : e} | page: ${page}` });
      throw e;
    }
  };

  try {
    await step("first-run setup accepts the default root folder", async () => {
      const yes = all(".lite-dialog button").find((b) => text(b) === "Yes");
      if (yes) yes.click();
      await waitFor("the setup dialog to close", () => !document.querySelector(".lite-dialog"));
    });
    await step("both panels show their counts", async () => {
      await waitFor("the Upload and Download panels", () => all(".lite-panel").length === 2 && all(".lite-count").every((c) => /\d/.test(text(c))));
    });
    await step("Download Changes lists and goes back", async () => {
      await click("Download Changes", () => button("Download Changes"));
      await waitFor("the download list", () => document.querySelector(".lite-list"));
      await click("Back", () => button("← Back"));
      await waitFor("the panels again", () => all(".lite-panel").length === 2);
    });
    await step("Upload Changes lists and goes back", async () => {
      await click("Upload Changes", () => button("Upload Changes"));
      await waitFor("the upload list", () => document.querySelector(".lite-list"));
      await click("Back", () => button("← Back"));
      await waitFor("the panels again", () => all(".lite-panel").length === 2);
    });
    await step("Settings opens and closes", async () => {
      await click("Settings", () => all(".lite-side button").find((b) => text(b) === "Settings"));
      await waitFor("the settings dialog", () => all(".lite-dialog h3").some((h) => text(h) === "Root folder"));
      await click("Done", () => button("Done"));
      await waitFor("settings to close", () => !document.querySelector(".lite-dialog"));
    });
    await step("New Project opens and cancels", async () => {
      await click("New Project", () => button("New Project"));
      await waitFor("the New Project dialog", () => all(".lite-dialog p").some((p) => text(p).startsWith("Paste a link")));
      await click("Cancel", () => button("Cancel"));
      await waitFor("New Project to close", () => !document.querySelector(".lite-dialog"));
    });
    await step("double-clicking a project loads it", async () => {
      const other = await waitFor("a second demo project", () => all(".project").find((p) => !p.classList.contains("active")));
      const name = text(other);
      other.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      await waitFor(`${name} to load`, () => text(document.querySelector(".lite-title") ?? document.body).startsWith(name));
      await waitFor("the panels", () => all(".lite-panel").length === 2);
    });
    await step("switching back to the first project", async () => {
      const first = await waitFor("the first demo project", () => all(".project").find((p) => text(p).startsWith("Demo Team Drive")));
      first.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      await waitFor("Demo Team Drive to load", () => text(document.querySelector(".lite-title") ?? document.body).startsWith("Demo Team Drive"));
    });
    await step("Leave demo goes back to Sign in", async () => {
      await click("Leave demo", () => button("Leave demo"));
      await waitFor("the sign-in screen", () => button("Sign in with Google") || all(".lite-empty strong").some((s) => text(s).startsWith("Sign in with Google")));
    });
  } catch {
    // The failed step is recorded; report what we have.
  }
  await api.smokeReport(steps);
}
