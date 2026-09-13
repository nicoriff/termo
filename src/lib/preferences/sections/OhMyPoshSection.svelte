<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { injectFontFace } from "$lib/theme/google-fonts";
  import {
    getOhMyPoshStatus,
    installNerdFont,
    installOhMyPosh,
    installOhMyPoshTheme,
    NERD_FONT_FAMILY,
    OH_MY_POSH_THEMES,
    type OhMyPoshStatus,
  } from "$lib/core/oh-my-posh-client";

  let status = $state<OhMyPoshStatus | null>(null);
  let busy = $state<"binary" | "theme" | "font" | null>(null);
  let error = $state<string | null>(null);

  async function refresh() {
    status = await getOhMyPoshStatus();
  }

  onMount(() => {
    void refresh();
  });

  async function run(kind: "binary" | "theme" | "font", task: () => Promise<void>) {
    busy = kind;
    error = null;
    try {
      await task();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = null;
    }
  }

  function install() {
    void run("binary", async () => {
      await installOhMyPosh();
      await installOhMyPoshTheme(appConfigStore.ohMyPoshTheme);
      await refresh();
    });
  }

  function pickTheme(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const theme = select.value;
    void run("theme", async () => {
      try {
        await installOhMyPoshTheme(theme);
        appConfigStore.setOhMyPoshTheme(theme);
      } catch (err) {
        select.value = appConfigStore.ohMyPoshTheme;
        throw err;
      }
    });
  }

  function useNerdFont() {
    void run("font", async () => {
      const path = await installNerdFont();
      injectFontFace(NERD_FONT_FAMILY, path);
      themeStore.setFontFamily(NERD_FONT_FAMILY);
      appConfigStore.setInstalledFont(path);
    });
  }
</script>

<svelte:window onfocus={() => void refresh()} />

<section>
  <h3>Instalación</h3>
  {#if status === null}
    <p class="setting-hint">Buscando Oh My Posh...</p>
  {:else if status.path}
    <p class="status-line">
      Instalado{status.managed ? " por Termini" : ""}
      <code>{status.path}</code>
    </p>
  {:else}
    <div class="row">
      <span>No está instalado</span>
      <button class="action-btn" onclick={install} disabled={busy !== null}>
        {busy === "binary" ? "Descargando..." : "Instalar"}
      </button>
    </div>
    <p class="setting-hint">
      Descarga el binario oficial desde cdn.ohmyposh.dev a la carpeta de Termini. No modifica tu PATH ni tus archivos
      de configuración.
    </p>
  {/if}
</section>

<section>
  <h3>Prompt</h3>
  <label class="toggle-row">
    <span>Usar Oh My Posh en sesiones nuevas</span>
    <input
      type="checkbox"
      checked={appConfigStore.ohMyPoshEnabled}
      disabled={!status?.path}
      onchange={(e) => appConfigStore.setOhMyPoshEnabled(e.currentTarget.checked)}
    />
  </label>
  <div class="row">
    <span>Tema</span>
    <select value={appConfigStore.ohMyPoshTheme} onchange={pickTheme} disabled={busy !== null || !status?.path}>
      {#each OH_MY_POSH_THEMES as theme (theme)}
        <option value={theme}>{theme}</option>
      {/each}
    </select>
  </div>
  <p class="setting-hint">
    Funciona en PowerShell, zsh y bash 4.2 o superior.{isMac
      ? " El bash que trae macOS (3.2) no es compatible: elegí zsh en Consola."
      : ""} Solo afecta sesiones nuevas.
  </p>
</section>

<section>
  <h3>Fuente con íconos</h3>
  <div class="row">
    <span>{NERD_FONT_FAMILY}</span>
    {#if themeStore.fontFamilyOverride === NERD_FONT_FAMILY}
      <span class="setting-hint">En uso</span>
    {:else}
      <button class="action-btn" onclick={useNerdFont} disabled={busy !== null}>
        {busy === "font" ? "Descargando..." : "Instalar y usar"}
      </button>
    {/if}
  </div>
  <p class="setting-hint">Los temas usan íconos de Nerd Fonts; sin esta fuente se ven como cuadraditos.</p>
</section>

{#if error}
  <p class="error">{error}</p>
{/if}

<style>
  section {
    margin-bottom: 20px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .row,
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 8px;
  }

  .toggle-row {
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .status-line {
    margin: 0;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  code {
    font-size: 10px;
    opacity: 0.7;
    word-break: break-all;
  }

  .action-btn,
  select {
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .action-btn:hover:not(:disabled),
  select:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  button:disabled,
  select:disabled {
    opacity: 0.5;
    cursor: default;
  }

  option {
    background: var(--tabbar-active-bg, #1e1e1e);
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .error {
    font-size: 11px;
    color: #f14c4c;
  }
</style>
