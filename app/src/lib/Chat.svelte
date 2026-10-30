<script lang="ts">
  import { onMount, tick } from "svelte";
  import { ask as askDialog } from "@tauri-apps/plugin-dialog";
  import { api, type Chat, type ChatFolder, type ChatMessage, type ChatProvider } from "./api";
  import { S } from "./store.svelte";
  import { ct, type CKey } from "./chatStrings";
  import ModelSelect from "./ModelSelect.svelte";

  const t = (k: CKey) => ct(S.lang, k);

  let providers = $state<ChatProvider[]>([]);
  let folders = $state<ChatFolder[]>([]);
  let chats = $state<Chat[]>([]);
  let query = $state("");
  let folderFilter = $state<string>("all"); // "all" | "" (no folder) | folder id
  let openId = $state("");
  let messages = $state<ChatMessage[]>([]);
  let text = $state("");
  let sending = $state(false);
  let err = $state("");
  let showSettings = $state(false);
  let box: HTMLDivElement | undefined = $state();

  // "Create video with chat": the assistant adds ```video {...}``` blocks; each becomes a card the user confirms.
  type Card = { workflow: string; prompt: string; params: Record<string, unknown> };
  let done = $state<Record<string, boolean>>({});
  function parse(content: string): { text: string; cards: Card[] } {
    const cards: Card[] = [];
    const text = content.replace(/```video\s*([\s\S]*?)```/g, (whole, body: string) => {
      try {
        const v = JSON.parse(body.trim());
        for (const c of Array.isArray(v) ? v : [v]) if (c && typeof c.workflow === "string" && typeof c.prompt === "string") cards.push({ workflow: c.workflow, prompt: c.prompt, params: c.params && typeof c.params === "object" ? c.params : {} });
      } catch { return whole; } // a broken block stays visible as plain text
      return "";
    }).trim();
    return { text, cards };
  }
  const wfOf = (id: string) => S.workflows.find((w) => w.meta.id === id);
  const wfName = (id: string) => { const w = wfOf(id); return w ? (S.lang === "el" ? w.meta.name_el : w.meta.name_en) : id; };
  function estimate(id: string) {
    const w = wfOf(id);
    if (!w) return 0;
    return w.meta.provider ? (w.meta.est_usd ?? 0) : S.dph > 0 ? (w.meta.est_sec * S.dph) / 3600 : 0;
  }
  async function generate(key: string, c: Card) {
    if (!wfOf(c.workflow)) return S.notify(t("bad_model") + " " + c.workflow, true);
    const j = await S.run(() => api.enqueue(S.projectId, c.workflow, c.prompt, c.params));
    if (j) { S.upsert(j); done[key] = true; S.notify(t("queued_card")); }
  }
  function editInGenerate(c: Card) { S.draft = { prompt: c.prompt, workflow: c.workflow }; S.page = "generate"; }

  const open = $derived(chats.find((c) => c.id === openId));
  const prov = $derived(providers.find((p) => p.id === open?.provider));
  const shown = $derived(query.trim() || folderFilter === "all" ? chats : chats.filter((c) => c.folder_id === folderFilter));

  async function reload() {
    chats = (await S.run(() => api.chatList(query))) ?? chats;
  }
  onMount(async () => {
    providers = (await S.run(api.chatProviders)) ?? [];
    folders = (await S.run(api.chatFolders)) ?? [];
    await reload();
    if (chats.length) await select(chats[0].id);
  });

  let timer: ReturnType<typeof setTimeout> | undefined;
  function onSearch() { clearTimeout(timer); timer = setTimeout(reload, 200); }

  async function select(id: string) {
    openId = id; err = ""; showSettings = false;
    messages = (await S.run(() => api.chatMessages(id))) ?? [];
    await scroll();
  }
  async function scroll() { await tick(); box?.scrollTo({ top: box.scrollHeight }); }

  async function newChat(mode: "chat" | "studio" = "chat") {
    const last = S.settings.chat_provider || providers.find((p) => p.has_key)?.id || "claude_sub";
    const c = await S.run(() => api.chatNew(folderFilter !== "all" ? folderFilter : "", last, S.settings["chat_model_" + last] ?? "", mode));
    if (!c) return;
    query = ""; await reload(); await select(c.id);
  }
  async function newFolder() {
    const name = prompt(t("folder_name"));
    if (name?.trim()) folders = (await S.run(() => api.chatFolderCreate(name))) ?? folders;
  }
  async function renameFolder(f: ChatFolder) {
    const name = prompt(t("rename"), f.name);
    if (name?.trim()) folders = (await S.run(() => api.chatFolderRename(f.id, name))) ?? folders;
  }
  async function deleteFolder(f: ChatFolder) {
    if (!(await askDialog(t("delete_folder_q"), { kind: "warning" }))) return;
    folders = (await S.run(() => api.chatFolderDelete(f.id))) ?? folders;
    if (folderFilter === f.id) folderFilter = "all";
    await reload();
  }
  async function save(c: Chat) { await S.run(() => api.chatUpdate(c)); await reload(); }
  async function remove(c: Chat) {
    if (!(await askDialog(t("delete_chat_q"), { kind: "warning" }))) return;
    await S.run(() => api.chatDelete(c.id));
    openId = ""; messages = []; await reload();
    if (chats.length) await select(chats[0].id);
  }
  function changeProvider(c: Chat, id: string) {
    c.provider = id;
    c.model = S.settings["chat_model_" + id] ?? providers.find((p) => p.id === id)?.model ?? "";
    S.set("chat_provider", id);
    save(c);
  }
  function changeModel(c: Chat) { S.set("chat_model_" + c.provider, c.model); save(c); }

  async function send() {
    if (!open || !text.trim() || sending) return;
    const msg = text; text = ""; sending = true; err = "";
    messages = [...messages, { id: "tmp", role: "user", content: msg, created_at: 0 }];
    await scroll();
    try {
      messages = await api.chatSend(open.id, msg);
    } catch (e) {
      err = String(e);
      messages = (await api.chatMessages(open.id).catch(() => messages)) as ChatMessage[];
      text = msg;
    }
    sending = false;
    await reload(); await scroll();
  }
  function key(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); }
  }
  async function copy(s: string) { await navigator.clipboard.writeText(s).catch(() => {}); S.notify(S.t("copied")); }
  function toGenerate(s: string) { S.draft = { prompt: s.replace(/^["“]|["”]$/g, "").trim(), workflow: "" }; S.page = "generate"; S.notify(t("prompt_sent")); }
</script>

<div class="chat">
  <aside>
    <div class="row top">
      <button class="primary grow" onclick={() => newChat("studio")}>🎬 {t("new_studio")}</button>
    </div>
    <div class="row top">
      <button class="grow" onclick={() => newChat("chat")}>＋ {t("new_chat")}</button>
    </div>
    <input class="srch" placeholder={t("search")} bind:value={query} oninput={onSearch} />
    <div class="folders">
      <button class="fl" class:on={folderFilter === "all"} onclick={() => (folderFilter = "all")}>{t("all")}</button>
      <button class="fl" class:on={folderFilter === ""} onclick={() => (folderFilter = "")}>{t("no_folder")}</button>
      {#each folders as f}
        <div class="fl row" class:on={folderFilter === f.id}>
          <button class="flb grow" onclick={() => (folderFilter = f.id)}>▸ {f.name}</button>
          <button class="ghost small" title={t("rename")} onclick={() => renameFolder(f)}>✎</button>
          <button class="ghost small" title={t("delete")} onclick={() => deleteFolder(f)}>✕</button>
        </div>
      {/each}
      <button class="ghost small" onclick={newFolder}>＋ {t("new_folder")}</button>
    </div>
    <div class="list">
      {#each shown as c (c.id)}
        <button class="item" class:on={c.id === openId} onclick={() => select(c.id)}>
          <span class="ttl">{c.pinned ? "📌 " : ""}{c.mode === "studio" ? "🎬 " : ""}{c.title}</span>
          {#if c.snippet}<span class="snip">{c.snippet}</span>{/if}
        </button>
      {/each}
      {#if query.trim()}<div class="small dim" style="padding:6px">{shown.length} {t("results")}</div>{/if}
    </div>
  </aside>

  <section>
    {#if open}
      <header class="row">
        <input class="title" value={open.title} onchange={(e) => { open!.title = e.currentTarget.value; save(open!); }} />
        <select value={open.provider} onchange={(e) => changeProvider(open!, e.currentTarget.value)} style="width:auto">
          {#each providers as p}<option value={p.id}>{p.label}{p.has_key ? "" : " · " + t("no_key_badge")}</option>{/each}
        </select>
        <span class="model"><ModelSelect provider={open.provider} value={open.model} onchange={(v) => { open!.model = v; changeModel(open!); }} /></span>
        <button class="ghost" onclick={() => (showSettings = !showSettings)} title={t("system")}>⚙</button>
        <button class="ghost" onclick={() => { open!.pinned = !open!.pinned; save(open!); }}>{open.pinned ? t("unpin") : t("pin")}</button>
        <button class="ghost danger" onclick={() => remove(open!)}>{t("delete")}</button>
      </header>
      {#if showSettings}
        <div class="card" style="margin:8px 0">
          <div class="field"><label for="sys">{t("system")}</label><textarea id="sys" rows="3" value={open.system} onchange={(e) => { open!.system = e.currentTarget.value; save(open!); }}></textarea></div>
          <div class="field"><label for="mv">{t("move")}</label>
            <select id="mv" value={open.folder_id} onchange={(e) => { open!.folder_id = e.currentTarget.value; save(open!); }}>
              <option value="">{t("no_folder")}</option>
              {#each folders as f}<option value={f.id}>{f.name}</option>{/each}
            </select>
          </div>
        </div>
      {/if}
      {#if prov && !prov.has_key}<div class="msg warn">{prov.id.endsWith("_sub") ? t("need_cli") : t("need_key")}</div>{/if}

      <div class="msgs" bind:this={box}>
        {#if !messages.length}<p class="dim empty">{open.mode === "studio" ? t("studio_empty") : t("empty")}</p>{/if}
        {#each messages as m (m.id)}
            {@const p = m.role === "assistant" && open.mode === "studio" ? parse(m.content) : { text: m.content, cards: [] as Card[] }}
          <div class="m {m.role}">
            <div class="who small dim">{m.role === "user" ? t("you") : t("assistant")}</div>
            {#if p.text}<div class="body">{p.text}</div>{/if}
            {#each p.cards as c, i}
              {@const k = m.id + ":" + i}
              <div class="card vcard">
                <div class="row"><b class="grow">{wfName(c.workflow)}</b>{#if estimate(c.workflow) > 0}<span class="small dim">{t("est")} ~${estimate(c.workflow).toFixed(2)}</span>{/if}</div>
                <p class="vprompt">{c.prompt}</p>
                {#if Object.keys(c.params).length}<p class="small dim">{Object.entries(c.params).map(([a, b]) => `${a}: ${b}`).join(" · ")}</p>{/if}
                <div class="row">
                  {#if done[k]}<span class="small">✓ {t("queued_card")}</span>
                  {:else}
                    <button class="primary" onclick={() => generate(k, c)}>▶ {t("gen_now")}</button>
                    <button class="ghost" onclick={() => editInGenerate(c)}>{t("edit_in_gen")}</button>
                  {/if}
                </div>
              </div>
            {/each}
            {#if m.role === "assistant" && open.mode !== "studio"}
              <div class="acts">
                <button class="ghost small" onclick={() => copy(m.content)}>{t("copy")}</button>
                <button class="ghost small" onclick={() => toGenerate(m.content)}>✦ {t("to_generate")}</button>
              </div>
            {/if}
          </div>
        {/each}
        {#if sending}<div class="m assistant"><div class="body dim">{t("thinking")}</div></div>{/if}
      </div>
      {#if err}<div class="msg err">{err}</div>{/if}
      <div class="composer row">
        <textarea rows="2" bind:value={text} placeholder={open.mode === "studio" ? t("studio_placeholder") : t("placeholder")} onkeydown={key}></textarea>
        <button class="primary" disabled={sending || !text.trim()} onclick={send}>{t("send")}</button>
      </div>
    {:else}
      <p class="dim empty">{t("empty")}</p>
    {/if}
  </section>
</div>

<style>
  .chat { display: grid; grid-template-columns: 290px 1fr; height: 100%; }
  aside { border-right: 1px solid var(--line); padding: 12px; display: flex; flex-direction: column; gap: 8px; min-height: 0; background: var(--panel); }
  .row { display: flex; gap: 8px; align-items: center; } .grow { flex: 1; }
  .folders { display: flex; flex-direction: column; gap: 2px; }
  .fl { text-align: left; background: transparent; border: 0; padding: 5px 8px; border-radius: 8px; color: var(--dim); }
  .fl.on { background: var(--sel); color: var(--text); }
  .flb { background: transparent; border: 0; text-align: left; color: inherit; padding: 0; }
  .list { overflow-y: auto; display: flex; flex-direction: column; gap: 2px; min-height: 0; flex: 1; }
  .item { text-align: left; display: flex; flex-direction: column; gap: 2px; background: transparent; border: 0; padding: 8px 10px; border-radius: 8px; }
  .item:hover { background: var(--panel2); } .item.on { background: var(--sel); }
  .ttl { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .snip { font-size: 11px; color: var(--dim); overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
  section { display: flex; flex-direction: column; padding: 12px 18px; min-width: 0; min-height: 0; }
  header { padding-bottom: 8px; border-bottom: 1px solid var(--line); }
  .title { font-weight: 600; flex: 1; min-width: 80px; } .model { width: 170px; }
  .msgs { flex: 1; overflow-y: auto; padding: 14px 4px; display: flex; flex-direction: column; gap: 14px; min-height: 0; }
  .m { max-width: 820px; } .m.user { align-self: flex-end; } .m.user .body { background: var(--sel); }
  .body { white-space: pre-wrap; word-break: break-word; background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 10px 13px; line-height: 1.5; }
  .vcard { margin-top: 6px; display: flex; flex-direction: column; gap: 6px; } .vprompt { margin: 0; white-space: pre-wrap; }
  .acts { display: flex; gap: 4px; margin-top: 2px; }
  .composer { align-items: flex-end; padding-top: 8px; } .composer textarea { resize: none; }
  .empty { padding: 30px; max-width: 46ch; }
</style>
