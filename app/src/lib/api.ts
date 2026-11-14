import { invoke } from "@tauri-apps/api/core";

export type InputDef = {
  key: string; type: "text" | "int" | "float" | "seed" | "image"; default?: unknown;
  min?: number; max?: number; step?: number; required?: boolean; advanced?: boolean;
  label_el: string; label_en: string; options?: string[];
};
export type Workflow = {
  meta: {
    id: string; name_el: string; name_en: string; desc_el: string; desc_en: string;
    kind: string; models: string[]; vram_gb: number; est_sec: number; pack?: string; priority?: number; inputs: InputDef[]; provider?: string; model?: string; est_usd?: number; license?: string; good_en?: string; good_el?: string; languages?: string[]; notice_en?: string; notice_el?: string;
  };
  source: string; file: string;
};
export type Project = { id: string; name: string; created_at: number; folder: string; suffix: string; sidecar: boolean; dir: string };
export type JobStatus = "queued" | "routing" | "generating" | "downloading" | "done" | "failed" | "cancelled";
export type Job = {
  id: string; project_id: string; workflow_id: string; prompt: string; params: string;
  status: JobStatus; message: string; error: string; file_path: string;
  created_at: number; started_at: number; finished_at: number; est_cost: number; gpu: string;
};
export type ModelFile = { dir: string; url: string };
export type ModelPack = {
  id: string; name: string; kind: string; license: string; vram_gb: number; size_gb: number;
  desc_el: string; desc_en: string; in_template: boolean; files: ModelFile[]; custom?: boolean; repo?: string; insight?: Insight | null;
};
export type HfHit = { id: string; downloads: number; likes: number; tag: string };
export type HfFile = { path: string; size: number; dir: string; precision: string; recommended: boolean };
export type Insight = {
  kind: string; modes: string[]; family: string; params_b: number; precision: string; vram_gb: number;
  quality: number; speed: number; basis: string; commercial: boolean | null;
  typical: [string, string][]; workflows: string[]; too_many: boolean;
};
export type HfRepo = { id: string; license: string; gated: boolean; diffusers: boolean; files: HfFile[]; insight: Insight };
export type HfResult = { kind: "hits"; data: HfHit[] } | { kind: "repo"; data: HfRepo };
export type Bootstrap = {
  projects: Project[]; secrets: Record<string, boolean>; settings: Record<string, string>;
  workflows: Workflow[]; workflow_errors: string[]; user_workflows_dir: string; output_dir: string;
  anthropic_models: string[]; cli_models: string[]; claude_cli: string | null; models: ModelPack[];
};
export type EndpointInfo = {
  id: number; name: string; state: string; cold_workers: number; min_load: number;
  max_workers: number; scales_to_zero: boolean;
};
export type VastTest = { ok: boolean; message: string; endpoint: EndpointInfo | null; all_endpoints: string[] };
export type GpuOffer = { name: string; vram_gb: number; min_price: number; median_price: number; offers: number };
export type InstallResult = { script: string; download_gb: number; applied: boolean; message: string };
export type RunpodState = "asleep" | "awake" | "working";
export type RunpodStatus = {
  state: RunpodState; workers_max: number; endpoint_id: string; jobs_waiting: number; session_usd: number;
  usd_per_hour_while_working: number; auto_sleep_min: number; note: string;
};

export type Enhanced = { text: string; tokens: number | null; provider: string };

export type Hardware = {
  os: string; gpus: { name: string; vram_mb: number; driver: string }[]; ram_gb: number;
  disk_free_gb: number; disk_path: string; support: "nvidia" | "amd" | "intel" | "none";
};
export type LocalStatus = { installed: boolean; running: boolean; dir: string; port: number; models: string[] };
export type SetupProgress = { step: string; pct: number; detail: string };

export type ChatProvider = { id: string; label: string; model: string; needs_key: boolean; has_key: boolean };
export type ChatFolder = { id: string; name: string };
export type Chat = { id: string; folder_id: string; title: string; provider: string; model: string; system: string; pinned: boolean; created_at: number; updated_at: number; mode: "chat" | "studio"; snippet: string };
export type ChatMessage = { id: string; role: "user" | "assistant"; content: string; created_at: number };

export const api = {
  hostedTest: (provider: string) => invoke<string>("hosted_test", { provider }),
  chatProviders: () => invoke<ChatProvider[]>("chat_providers"),
  chatModels: (provider: string) => invoke<string[]>("chat_models", { provider }),
  chatFolders: () => invoke<ChatFolder[]>("chat_folders"),
  chatFolderCreate: (name: string) => invoke<ChatFolder[]>("chat_folder_create", { name }),
  chatFolderRename: (id: string, name: string) => invoke<ChatFolder[]>("chat_folder_rename", { id, name }),
  chatFolderDelete: (id: string) => invoke<ChatFolder[]>("chat_folder_delete", { id }),
  chatList: (query: string) => invoke<Chat[]>("chat_list", { query }),
  chatNew: (folderId: string, provider: string, model: string, mode: "chat" | "studio" = "chat") => invoke<Chat>("chat_new", { folderId, provider, model, mode }),
  chatUpdate: (c: Chat) => invoke<void>("chat_update", { id: c.id, title: c.title, folderId: c.folder_id, provider: c.provider, model: c.model, system: c.system, pinned: c.pinned }),
  chatDelete: (id: string) => invoke<void>("chat_delete", { id }),
  chatMessages: (id: string) => invoke<ChatMessage[]>("chat_messages", { id }),
  chatMessageDelete: (id: string) => invoke<void>("chat_message_delete", { id }),
  editorUrl: () => invoke<string | null>("editor_url"),
  inputImport: (path: string) => invoke<string>("input_import", { path }),
  paintUrl: () => invoke<string | null>("paint_url"),
  textOutput: (path: string) => invoke<string>("text_output", { path }),
  imageDataUrl: (path: string) => invoke<string>("image_data_url", { path }),
  chatSend: (id: string, text: string) => invoke<ChatMessage[]>("chat_send", { id, text }),
  hardwareScan: () => invoke<Hardware>("hardware_scan"),
  localStatus: () => invoke<LocalStatus>("local_status"),
  localInstall: () => invoke<void>("local_install"),
  localInstallModels: (ids: string[]) => invoke<void>("local_install_models", { ids }),
  setupCancel: () => invoke<void>("setup_cancel"),
  localStart: () => invoke<void>("local_start"),
  localStop: () => invoke<void>("local_stop"),
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  reloadWorkflows: () => invoke<[Workflow[], string[]]>("reload_workflows"),
  setSetting: (key: string, value: string) => invoke<void>("setting_set", { key, value }),
  setSecret: (name: string, value: string) => invoke<Record<string, boolean>>("secret_set", { name, value }),
  projectCreate: (name: string, folder: string) => invoke<Project[]>("project_create", { name, folder }),
  projectUpdate: (id: string, name: string, folder: string, suffix: string, sidecar: boolean) =>
    invoke<Project[]>("project_update", { id, name, folder, suffix, sidecar }),
  projectDelete: (id: string) => invoke<Project[]>("project_delete", { id }),
  jobs: () => invoke<Job[]>("jobs_list"),
  enqueue: (projectId: string, workflowId: string, prompt: string, params: Record<string, unknown>) =>
    invoke<Job>("job_enqueue", { projectId, workflowId, prompt, params }),
  cancel: (id: string) => invoke<void>("job_cancel", { id }),
  retry: (id: string) => invoke<void>("job_retry", { id }),
  deleteJob: (id: string, deleteFile: boolean) => invoke<void>("job_delete", { id, deleteFile }),
  enhance: (idea: string, hint: string) => invoke<Enhanced>("enhance_prompt", { idea, hint }),
  testVast: () => invoke<VastTest>("test_vast"),
  testLlm: () => invoke<string>("test_llm"),
  scaleToZero: () => invoke<string>("apply_scale_to_zero"),
  gpuOffers: (minVram: number) => invoke<GpuOffer[]>("gpu_offers", { minVram }),
  applyGpus: (gpus: string[], minVram: number, dph: number) => invoke<string>("apply_gpus", { gpus, minVram, dph }),
  vastInstances: () => invoke<{ id: number; gpu: string; status: string; usd_per_hour: number; minutes: number; cost_so_far: number }[]>("vast_instances"),
  vastDestroy: (id: number) => invoke<string>("vast_destroy", { id }),
  warmUp: (image = false, extra = false) => invoke<void>("warm_up", { image, extra }),
  warmCancel: () => invoke<void>("warm_cancel"),
  installOnWorkers: (ids: string[], apply: boolean) => invoke<InstallResult>("install_on_workers", { ids, apply }),
  setupEndpoint: (gpus: string[]) => invoke<string>("setup_endpoint", { gpus }),
  runpodStatus: () => invoke<RunpodStatus>("runpod_status"),
  runpodWake: () => invoke<string>("runpod_wake"),
  runpodSleep: () => invoke<string>("runpod_sleep"),
  hfLookup: (query: string, videoOnly: boolean) => invoke<HfResult>("hf_lookup", { query, videoOnly }),
  customModelAdd: (repo: string, name: string, kind: string, vramGb: number, license: string, files: [string, string, number][], insight: Insight | null) =>
    invoke<ModelPack[]>("custom_model_add", { repo, name, kind, vramGb, license, files, insight }),
  customModelRemove: (id: string) => invoke<ModelPack[]>("custom_model_remove", { id }),
  workflowImport: (path: string) => invoke<[Workflow[], string[]]>("workflow_import", { path }),
  logsTail: (lines: number) => invoke<string>("logs_tail", { lines }),
  logUi: (level: string, msg: string) => invoke<void>("log_ui", { level, msg }),
  logsReport: () => invoke<string>("logs_report"),
  logsDir: () => invoke<string>("logs_dir"),
  connectInfo: () => invoke<{ extension: string; installer_bat: string; exe: string; claude_code_cmd: string; folder: string }>("connect_info"),
  connectClients: () => invoke<string[]>("connect_clients"),
  reveal: (path: string) => invoke<void>("reveal", { path }),
  exportSettings: (path: string) => invoke<void>("export_settings", { path }),
  importSettings: (path: string) => invoke<void>("import_settings", { path }),
};
