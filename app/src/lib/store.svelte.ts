import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { ct } from "./chatStrings";
import { api, type Bootstrap, type Job, type Project, type RunpodStatus, type Workflow } from "./api";
import { tr, type Key, type Lang } from "./i18n";

export type Theme = "dark" | "light" | "system";
export type Page = "generate" | "chat" | "queue" | "library" | "projects" | "models" | "guide" | "editor" | "logs" | "settings";

class Store {
  ready = $state(false);
  wizard = $state(false);
  page = $state<Page>("generate");
  lang = $state<Lang>("el");
  boot = $state<Bootstrap | null>(null);
  projects = $state<Project[]>([]);
  projectId = $state("");
  workflows = $state<Workflow[]>([]);
  workflowErrors = $state<string[]>([]);
  jobs = $state<Job[]>([]);
  secrets = $state<Record<string, boolean>>({});
  settings = $state<Record<string, string>>({});
  toast = $state<{ text: string; err: boolean } | null>(null);
  draft = $state<{ prompt: string; workflow: string } | null>(null);
  private listening = false;
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  theme = $state<Theme>("dark");

  applyTheme(t: Theme = this.theme) {
    this.theme = t;
    const light = t === "light" || (t === "system" && window.matchMedia("(prefers-color-scheme: light)").matches);
    document.documentElement.dataset.theme = light ? "light" : "dark";
  }

  t = (k: Key) => tr(this.lang, k);

  /** Where open-source modes run by default: RunPod (cloud, automatic) unless the user chose this PC. */
  get provider(): "runpod" | "local" {
    return this.settings.provider === "local" ? "local" : "runpod";
  }
  /** RunPod needs both the API key and the endpoint id. */
  get runpodReady(): boolean {
    return !!this.secrets.runpod_api_key && !!(this.settings.runpod_endpoint_id ?? "").trim();
  }
  /** The mode that really runs: an open-source mode becomes its RunPod twin ("runpod-<id>") when the job runs on RunPod. */
  effective(id: string, provider: string = this.provider): Workflow | undefined {
    const w = this.workflows.find((x) => x.meta.id === id);
    if (w && !w.meta.provider && provider === "runpod") return this.workflows.find((x) => x.meta.id === "runpod-" + id && x.meta.provider === "runpod") ?? w;
    return w;
  }
  /** Typical price of one result in USD (0 = free, it runs on this PC). */
  estimate(id: string, provider: string = this.provider): number {
    return this.effective(id, provider)?.meta.est_usd ?? 0;
  }

  // RunPod status for the header chip and the RunPod panels; refreshed every 20 s and after every wake / sleep
  rp = $state<RunpodStatus | null>(null);
  rpError = $state("");
  rpBusy = $state(false);
  private rpTimer: ReturnType<typeof setInterval> | undefined;
  async refreshRunpod() {
    if (!this.runpodReady) { this.rp = null; this.rpError = ""; return; }
    try { this.rp = await api.runpodStatus(); this.rpError = ""; } catch (e) { this.rp = null; this.rpError = String(e); }
  }
  async runpodWake() {
    this.rpBusy = true;
    try { this.notify(await api.runpodWake()); } catch (e) { this.notify(String(e), true); }
    this.rpBusy = false;
    await this.refreshRunpod();
  }
  async runpodSleepAsk() {
    if (this.rp?.state === "working" && !(await ask(ct(this.lang, "rp_sleep_confirm"), { kind: "warning" }))) return;
    await this.runpodSleep();
  }
  async runpodSleep() {
    this.rpBusy = true;
    try { this.notify(await api.runpodSleep()); } catch (e) { this.notify(String(e), true); }
    this.rpBusy = false;
    await this.refreshRunpod();
  }
  get active(): Job | undefined {
    return this.jobs.find((j) => ["routing", "generating", "downloading"].includes(j.status));
  }
  get pending(): Job[] {
    return this.jobs.filter((j) => ["queued", "routing", "generating", "downloading"].includes(j.status));
  }
  get project(): Project | undefined {
    return this.projects.find((p) => p.id === this.projectId);
  }

  async init() {
    const b = await api.bootstrap();
    this.boot = b;
    this.projects = b.projects;
    this.workflows = b.workflows;
    this.workflowErrors = b.workflow_errors;
    this.secrets = b.secrets;
    this.settings = b.settings;
    this.lang = (b.settings.lang as Lang) || "en";
    this.applyTheme((b.settings.theme as Theme) || "dark");
    window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => this.theme === "system" && this.applyTheme());
    const last = b.settings.last_project;
    this.projectId = b.projects.find((p) => p.id === last)?.id ?? b.projects[0].id;
    this.jobs = await api.jobs();
    if (!this.listening) {
      this.listening = true;
      await listen<Job>("job-update", (e) => { this.upsert(e.payload); this.refreshRunpod(); });
      await listen("runpod-slept", () => this.refreshRunpod());
      this.rpTimer = setInterval(() => this.refreshRunpod(), 20000);
    }
    // first run: the setup wizard. People who already use RunPod skip it.
    if (!this.ready && !b.settings.setup_done && !this.secrets.runpod_api_key) this.wizard = true;
    else if (!this.ready && !this.secrets.runpod_api_key && b.settings.provider !== "local") this.page = "settings";
    this.ready = true;
    this.refreshRunpod();
  }

  upsert(j: Job) {
    const i = this.jobs.findIndex((x) => x.id === j.id);
    if (i >= 0) this.jobs[i] = j;
    else this.jobs.unshift(j);
  }

  async set(key: string, value: string) {
    this.settings[key] = value;
    await api.setSetting(key, value);
  }

  notify(text: string, err = false) {
    this.toast = { text, err };
    if (err) api.logUi("error", text).catch(() => {});
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), err ? 7000 : 3000);
  }

  async run<T>(f: () => Promise<T>): Promise<T | undefined> {
    try {
      return await f();
    } catch (e) {
      this.notify(String(e), true);
    }
  }
}

export const S = new Store();
