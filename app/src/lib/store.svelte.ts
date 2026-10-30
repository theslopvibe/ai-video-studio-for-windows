import { listen } from "@tauri-apps/api/event";
import { api, type Bootstrap, type Job, type Project, type Workflow } from "./api";
import { tr, type Key, type Lang } from "./i18n";

export type Theme = "dark" | "light" | "system";
export type Page = "generate" | "chat" | "queue" | "library" | "projects" | "gpu" | "models" | "guide" | "logs" | "settings";

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

  get dph(): number {
    return parseFloat(this.settings.dph_estimate ?? "") || 0;
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
      await listen<Job>("job-update", (e) => this.upsert(e.payload));
    }
    // first run: the setup wizard. People who already use a Vast key skip it.
    if (!this.ready && !b.settings.setup_done && !this.secrets.vast_api_key) this.wizard = true;
    else if (!this.ready && !this.secrets.vast_api_key && b.settings.provider !== "local") this.page = "settings";
    this.ready = true;
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
