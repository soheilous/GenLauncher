<script lang="ts">
  import { onDestroy } from "svelte";
  import { app } from "$lib/app.svelte";
  import {
    closeBuiltInBrowser, isTauri, onBrowserEvent, openBuiltInBrowser, openInBrowser, pickArchiveFile, pickModFolder,
    type BrowserEvent,
  } from "$lib/backend";
  import {
    formatBytes, guessFromFileName, guessNameFromPage, isModDbPage, isWebAddress, looksLikeDirectFile, slugify,
  } from "$lib/guess";
  import type { ModKind } from "$lib/types";
  import Icon from "./Icon.svelte";

  type Source =
    | { type: "file"; path: string; pageUrl: string | null }
    | { type: "link"; url: string }
    /** the file the built-in browser is downloading in the background */
    | { type: "browser"; pageUrl: string | null };

  let step = $state<"source" | "browser" | "details">("source");
  let tab = $state<"file" | "link">("file");
  let source = $state<Source | null>(null);
  let url = $state("");
  let pageUrl = $state<string | null>(null); // the web page the file came from
  /** What the built-in browser is doing: waiting for the user's click, or receiving a file. */
  let browser = $state<{ fileName: string | null; bytes: number }>({ fileName: null, bytes: 0 });
  let browserDone = false; // we closed the window ourselves (cancel / switch browser): not an error
  let downloadDone = $state(false); // the file has fully arrived
  let submitted = false; // the details were confirmed; the launcher now owns the download

  let name = $state("");
  let version = $state("1.0");
  let kind = $state<ModKind>("mod");
  let parent = $state("");
  let error = $state("");
  let busy = $state(false);

  let dialog: HTMLDivElement | undefined = $state();

  // --- does this name (and version) clash with something already there? ---
  // The launcher identifies a mod by its name, so a new "Zero Hour Reborn" would silently become another
  // version of the one you already have. The setup step therefore says so and asks.
  let ackSameName = $state(false);
  /** Opened from a catalog entry's Install/Update: adding another version of it is what was asked for. */
  const isUpdate = $derived(!!app.addDialog?.preset?.update);
  const targetId = $derived(kind === "mod" ? slugify(name) : `${parent}/${kind}/${slugify(name)}`);
  const sameItem = $derived(slugify(name) ? app.items.find((i) => i.id === targetId) : undefined);
  const installedVersions = $derived(sameItem?.versions.filter((v) => v.installed).map((v) => v.v) ?? []);
  const sameVersion = $derived(installedVersions.some((v) => v.trim().toLowerCase() === version.trim().toLowerCase()));
  const reserved = $derived(kind === "mod" && slugify(name) === "original-game");
  const inFlight = $derived(!!app.installProgress[targetId]);
  /** Adding must wait for the user: nothing to confirm, or a clash that is not acknowledged yet. */
  const blocked = $derived(reserved || inFlight || (!!sameItem && (sameVersion || !(ackSameName || isUpdate))));
  const suggestedName = $derived(`${name.trim()} ${version.trim()}`.trim());
  $effect(() => {
    targetId; // a different name, type or parent starts the question over
    ackSameName = false;
  });
  function useDifferentName() {
    name = suggestedName;
  }

  // (re)initialise whenever the dialog is opened
  $effect(() => {
    if (!app.addDialog) return;
    step = "source";
    tab = "file";
    source = null;
    url = "";
    pageUrl = null;
    browser = { fileName: null, bytes: 0 };
    browserDone = false;
    downloadDone = false;
    submitted = false;
    error = "";
    busy = false;
    const preset = app.addDialog.preset;
    name = preset?.name ?? "";
    version = "1.0";
    kind = app.addDialog.kind;
    parent = app.addDialog.parent ?? app.activeMod.id;
    queueMicrotask(() => dialog?.focus());
    if (preset?.pageUrl) {
      // the catalog knows this mod's page: open it right away so the user only has to pick the version and download
      tab = "link";
      url = preset.pageUrl;
      queueMicrotask(() => void continueWithLink());
    }
  });

  // events from the built-in browser window
  let unlisten: (() => void) | undefined;
  if (isTauri) void onBrowserEvent(handleBrowserEvent).then((u) => (unlisten = u));
  onDestroy(() => unlisten?.());

  /** While the page is open, and while the details are being filled in with the file still arriving. */
  function handleBrowserEvent(e: BrowserEvent) {
    if (!app.addDialog) return;
    const waiting = step === "browser";
    const watching = step === "details" && source?.type === "browser" && !submitted;
    if (!waiting && !watching) return;

    if (e.state === "started") {
      browser = { fileName: e.fileName, bytes: 0 };
      downloadDone = false;
      if (waiting) {
        // the download has begun (the page is already hidden): ask for the details right away,
        // the file keeps arriving in the background
        const fromFile = guessFromFileName(e.fileName);
        const fromPage = pageUrl && isModDbPage(pageUrl) ? guessNameFromPage(pageUrl) : "";
        if (!name.trim()) name = fromPage || fromFile.name;
        toDetails({ name, version: fromFile.version }, { type: "browser", pageUrl });
      }
    } else if (e.state === "progress") {
      browser = { fileName: e.fileName, bytes: e.bytes };
    } else if (e.state === "finished") {
      downloadDone = true;
    } else if (e.state === "failed" || e.state === "stalled") {
      backToLink(e.message); // the launcher already closed the window
    } else if (e.state === "closed" && !downloadDone && !browserDone) {
      backToLink("The browser window was closed before the download finished.");
    }
  }

  function backToLink(message: string) {
    browserDone = true;
    source = null;
    step = "source";
    tab = "link";
    error = message;
  }

  const parentName = $derived(app.mods.find((m) => m.id === parent)?.name ?? "");
  const title = $derived(
    kind === "mod" ? "Add a mod" : `Add ${kind === "exe" ? "an executable" : "a " + kind}${parentName ? " for " + parentName : ""}`
  );

  function toDetails(guess: { name: string; version: string }, src: Source) {
    source = src;
    if (!name.trim()) name = guess.name;
    version = guess.version;
    error = "";
    step = "details";
  }

  async function chooseFile() {
    error = "";
    // outside the app (browser preview) there is no file dialog: use a sample so the flow can be seen
    const path = isTauri ? await pickArchiveFile() : "C:\\Downloads\\Sample_Mod_v1.2.zip";
    if (path) toDetails(guessFromFileName(path), { type: "file", path, pageUrl });
  }

  async function chooseFolder() {
    error = "";
    const path = isTauri ? await pickModFolder() : "C:\\Downloads\\Sample Mod";
    if (path) toDetails({ ...guessFromFileName(path), version: "1.0" }, { type: "file", path, pageUrl });
  }

  async function continueWithLink() {
    const link = url.trim();
    if (!isWebAddress(link)) {
      error = "Paste a web address that starts with http:// or https://";
      return;
    }
    error = "";
    if (looksLikeDirectFile(link)) {
      toDetails(guessFromFileName(new URL(link).pathname), { type: "link", url: link });
      return;
    }
    // a web page (ModDB, a mod's website, ...): the user finds and clicks the download in the launcher's browser
    pageUrl = link;
    if (isModDbPage(link) && !name.trim()) name = guessNameFromPage(link);
    browser = { fileName: null, bytes: 0 };
    browserDone = false;
    downloadDone = false;
    submitted = false;
    step = "browser";
    try {
      if (isTauri) await openBuiltInBrowser(link);
      else simulateBrowser(); // preview only
    } catch (e) {
      step = "source";
      error = e instanceof Error ? e.message : String(e);
    }
  }

  /** Browser preview has no second window: play the events a real download would produce. */
  function simulateBrowser() {
    const fileName = "Sample_Mod_v1.2.zip";
    setTimeout(() => handleBrowserEvent({ state: "started", fileName }), 1200);
    for (const [at, bytes] of [[1900, 8_400_000], [2600, 23_100_000], [3300, 41_800_000]] as const) {
      setTimeout(() => handleBrowserEvent({ state: "progress", fileName, bytes }), at);
    }
    setTimeout(() => handleBrowserEvent({ state: "finished", fileName, path: "C:\\tmp\\" + fileName }), 4200);
  }

  async function cancelBrowser() {
    browserDone = true; // we close it ourselves: not an error
    if (isTauri) await closeBuiltInBrowser().catch(() => {});
    step = "source";
    tab = "link";
    error = "";
  }

  async function useSystemBrowserInstead() {
    browserDone = true;
    try {
      if (isTauri) {
        await closeBuiltInBrowser().catch(() => {});
        await openInBrowser(url.trim());
      } else window.open(url.trim(), "_blank");
    } catch {
      error = "Could not open your browser. Copy the address and open it yourself.";
    }
    step = "source";
    tab = "file"; // download it there, then choose the file here
  }

  async function submit() {
    if (!source) return;
    if (!name.trim() || !/[\p{L}\p{N}]/u.test(name)) {
      error = "Give it a name with at least one letter or number.";
      return;
    }
    if (!version.trim()) {
      error = "Enter a version, for example 1.0.";
      return;
    }
    if (kind !== "mod" && !parent) {
      error = "Choose which mod this belongs to.";
      return;
    }
    if (reserved) {
      error = "That name is reserved for the stock game. Choose another name.";
      return;
    }
    if (inFlight) {
      error = "Something with this name is already being installed. Wait for it to finish.";
      return;
    }
    if (sameItem && sameVersion) {
      error = `${sameItem.name} ${version.trim()} is already installed. Choose another version or remove the installed one first.`;
      return;
    }
    if (sameItem && !ackSameName && !isUpdate) {
      error = `You already have "${sameItem.name}". Tick the box if this is another version of it, or choose a different name.`;
      return;
    }
    error = "";
    busy = true;
    const request =
      source.type === "file" ? { type: "file" as const, path: source.path, pageUrl: source.pageUrl }
      : source.type === "link" ? { type: "link" as const, url: source.url, pageUrl }
      : { type: "browser" as const, pageUrl: source.pageUrl };
    // For a browser download this works whether or not the file has finished arriving: it joins the
    // downloads list and installs itself the moment the file is complete.
    const ok = await app.add({ source: request, name, version, kind, parent: kind === "mod" ? null : parent });
    busy = false;
    if (ok) {
      submitted = true;
      app.closeAdd();
    }
  }

  function close() {
    if (busy) return;
    // leaving before confirming throws the download away (a half-finished or finished file must not linger)
    const unconfirmed = step === "browser" || (step === "details" && source?.type === "browser" && !submitted);
    if (unconfirmed) void cancelBrowser();
    app.closeAdd();
  }

  const sourceLabel = $derived(
    source ? (source.type === "file" ? source.path : source.type === "link" ? source.url : (browser.fileName ?? "download from the page")) : ""
  );
</script>

<svelte:window onkeydown={(e) => { if (app.addDialog && e.key === "Escape") close(); }} />

{#if app.addDialog}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="glass dlg" role="dialog" aria-modal="true" aria-labelledby="add-title" tabindex="-1" bind:this={dialog}>
    <header>
      <h2 id="add-title">{title}</h2>
      <button class="btn" aria-label="Close" onclick={close}><Icon name="x" size={16} /></button>
    </header>

    {#if step === "source"}
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "file"} class:on={tab === "file"} onclick={() => (tab = "file")}>From a file</button>
        <button role="tab" aria-selected={tab === "link"} class:on={tab === "link"} onclick={() => (tab = "link")}>From a link</button>
      </div>

      {#if tab === "file"}
        <p class="hint">
          {#if pageUrl}Now choose the file you downloaded from the page.{:else}Choose the archive you downloaded (zip, 7z, rar), a single .big file, or a folder with the mod's files.{/if}
        </p>
        <div class="choices">
          <button class="choice" onclick={chooseFile}><Icon name="download" size={22} /><strong>Choose a file…</strong><small>zip · 7z · rar · big</small></button>
          <button class="choice" onclick={chooseFolder}><Icon name="folder" size={22} /><strong>Choose a folder…</strong><small>already unpacked</small></button>
        </div>
      {:else}
        <label class="field">
          <span>Web address</span>
          <input type="url" placeholder="https://www.moddb.com/mods/…  or a direct download link" bind:value={url}
            onkeydown={(e) => e.key === "Enter" && continueWithLink()} spellcheck="false" />
        </label>
        <p class="hint">
          Paste a mod's page (ModDB or any website) and the launcher opens it in its own browser window: you choose the
          version and click its download button, and the launcher takes the file. A direct link to a file (OneDrive,
          Dropbox, GitHub, <code>.zip</code>) is downloaded straight away.
        </p>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <footer>
        <button class="btn" onclick={close}>Cancel</button>
        {#if tab === "link"}<button class="btn primary" onclick={continueWithLink}>Continue</button>{/if}
      </footer>
    {:else if step === "browser"}
      <div class="waiting" aria-live="polite">
        <div class="spinner" aria-hidden="true"></div>
        {#if browser.fileName}
          <strong>Downloading {browser.fileName}</strong>
          <p>{browser.bytes > 0 ? formatBytes(browser.bytes) + " received" : "Starting…"}. The page has been put away; the launcher continues by itself when the download is complete.</p>
        {:else}
          <strong>Waiting for you in the browser window</strong>
          <p>A window with the page has opened. Choose the version you want and click its download button there. The launcher takes the file as soon as the download starts.</p>
        {/if}
      </div>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <footer>
        <button class="btn" onclick={useSystemBrowserInstead}>Use my own browser instead</button>
        <button class="btn" onclick={cancelBrowser}>Cancel</button>
      </footer>
    {:else}
      <p class="from" title={sourceLabel}><Icon name={source?.type === "file" ? "folder" : "link"} size={15} /> <span>{sourceLabel}</span></p>

      {#if source?.type === "browser"}
        <div class="dl" class:done={downloadDone} aria-live="polite">
          {#if downloadDone}
            <Icon name="check" size={16} /> <span>Download complete{browser.bytes ? ` (${formatBytes(browser.bytes)})` : ""}. Confirm to install it.</span>
          {:else}
            <div class="spinner small" aria-hidden="true"></div>
            <span>Downloading in the background{browser.bytes > 0 ? ` · ${formatBytes(browser.bytes)} received` : "…"}. You can fill this in while it arrives.</span>
          {/if}
        </div>
      {/if}

      <div class="form">
        <label class="field">
          <span>Name</span>
          <input bind:value={name} spellcheck="false" onkeydown={(e) => e.key === "Enter" && submit()} />
        </label>
        <label class="field narrow">
          <span>Version</span>
          <input bind:value={version} spellcheck="false" onkeydown={(e) => e.key === "Enter" && submit()} />
        </label>
      </div>
      {#if reserved}
        <div class="alert danger" role="alert"><strong>"Original Game" is reserved for the stock game.</strong> Choose another name.</div>
      {:else if inFlight}
        <div class="alert danger" role="alert"><strong>Something with this name is already being installed.</strong> Wait for it to finish, or choose another name.</div>
      {:else if sameItem && sameVersion}
        <div class="alert danger" role="alert">
          <strong>{sameItem.name} {version.trim()} is already installed.</strong>
          <p>Choose another version, or remove the installed one first. If this is a different mod, give it a different name.</p>
          <div class="row"><button class="btn" onclick={useDifferentName}>Use "{suggestedName}"</button></div>
        </div>
      {:else if sameItem && !isUpdate}
        <div class="alert warn" role="alert">
          <strong>You already have a {sameItem.kind === "mod" ? "mod" : sameItem.kind} called "{sameItem.name}"</strong>
          ({installedVersions.length ? "installed: " + installedVersions.join(", ") : "in the catalog, not installed"}).
          <p>
            If you continue, this is stored as <b>another version of it</b> and appears in its version list. If it is a
            different mod (another page or another author), give it a different name instead.
          </p>
          <div class="row">
            <button class="btn" onclick={useDifferentName}>Use "{suggestedName}"</button>
            <label class="check"><input type="checkbox" bind:checked={ackSameName} /> It is another version of {sameItem.name}</label>
          </div>
        </div>
      {/if}
      <p class="hint">
        The name and version were guessed from the file. Change them if they're wrong; they decide where the files are stored.
        {#if source?.type === "browser" && !downloadDone}Once you confirm, it is added to Downloads and installs by itself as soon as the file is complete.{/if}
      </p>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <footer>
        {#if source?.type !== "browser"}<button class="btn" disabled={busy} onclick={() => (step = "source")}>Back</button>{/if}
        {#if source?.type === "browser"}<button class="btn" disabled={busy} onclick={close}>Cancel download</button>{/if}
        <button class="btn primary" disabled={busy || blocked} onclick={submit}><Icon name="plus" size={16} /> Add {kind === "mod" ? "mod" : kind === "exe" ? "executable" : kind}</button>
      </footer>
    {/if}
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; background: var(--scrim); backdrop-filter: blur(4px); z-index: 20; animation: fade .2s; }
  .dlg { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(560px, calc(100vw - 32px));
    padding: 22px; background: var(--surface-solid); animation: pop .25s var(--ease); outline: none; max-height: calc(100vh - 32px); overflow: auto; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; }
  h2 { font-size: 20px; }
  .tabs { display: flex; gap: 4px; padding: 4px; background: var(--surface); border: 1px solid var(--stroke); border-radius: 999px; margin-bottom: 14px; }
  .tabs button { flex: 1; padding: 8px; border-radius: 999px; font-weight: 600; color: var(--text-dim); transition: background .2s, color .2s; }
  .tabs button.on { background: var(--accent); color: var(--accent-ink); }
  .hint { color: var(--text-dim); font-size: 13px; margin: 10px 0 0; }
  .hint code { background: var(--surface-hi); padding: 1px 5px; border-radius: 6px; }
  .choices { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-top: 14px; }
  .choice { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 22px 12px; border-radius: var(--r-md);
    background: var(--surface); border: 1px dashed var(--stroke-hi); transition: background .15s, border-color .15s, transform .2s var(--ease); }
  .choice:hover { background: var(--accent-soft); border-color: var(--accent); transform: translateY(-2px); }
  .choice:focus-visible, .tabs button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .choice small { color: var(--text-faint); }
  .field { display: flex; flex-direction: column; gap: 6px; margin-top: 12px; }
  .field > span { font-size: 12px; color: var(--text-faint); }
  .field.narrow { max-width: 200px; }
  input { background: var(--surface-hi); border: 1px solid var(--stroke); border-radius: 999px; padding: 10px 16px; outline: none; user-select: text; width: 100%; }
  input:focus { border-color: var(--accent); }
  .form { display: flex; flex-direction: column; }
  .form :global(.sel) { flex: none; width: 100%; }
  .from { display: flex; align-items: center; gap: 8px; color: var(--text-dim); font-size: 13px; margin: 0 0 4px; min-width: 0; }
  .from span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .waiting { display: flex; flex-direction: column; align-items: center; text-align: center; gap: 8px; padding: 26px 12px; border-radius: var(--r-md); background: var(--accent-soft); border: 1px solid var(--stroke); }
  .waiting p { margin: 0; color: var(--text-dim); font-size: 13px; max-width: 440px; }
  .spinner { width: 30px; height: 30px; border-radius: 50%; border: 3px solid var(--stroke-hi); border-top-color: var(--accent); animation: spin 0.9s linear infinite; flex: none; }
  .spinner.small { width: 16px; height: 16px; border-width: 2px; }
  .dl { display: flex; align-items: center; gap: 10px; padding: 10px 14px; margin-bottom: 4px; border-radius: var(--r-md); background: var(--accent-soft); border: 1px solid var(--stroke); font-size: 13px; color: var(--text-dim); }
  .dl.done { color: var(--ok); }
  .error { color: var(--danger); font-size: 13px; margin: 10px 0 0; }
  .alert { margin-top: 12px; padding: 12px 14px; border-radius: var(--r-md); font-size: 13px; color: var(--text-dim); border: 1px solid var(--stroke); }
  .alert strong { color: var(--text); }
  .alert p { margin: 6px 0 10px; }
  .alert.warn { background: color-mix(in srgb, var(--warn) 14%, transparent); border-color: color-mix(in srgb, var(--warn) 45%, transparent); }
  .alert.danger { background: color-mix(in srgb, var(--danger) 14%, transparent); border-color: color-mix(in srgb, var(--danger) 45%, transparent); }
  .row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .check { display: inline-flex; align-items: center; gap: 8px; cursor: pointer; color: var(--text); }
  .check input { width: 16px; height: 16px; padding: 0; accent-color: var(--accent); border-radius: 4px; }
  footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -46%) scale(.97); } }
</style>
