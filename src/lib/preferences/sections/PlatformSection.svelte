<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac, isWindows } from "$lib/core/platform";

  let { os }: { os: "mac" | "windows" | "linux" } = $props();

  const currentOs = isMac ? "mac" : isWindows ? "windows" : "linux";
  const active = $derived(os === currentOs);
</script>

<div class="platform-section" class:disabled={!active}>
  {#if !active}
    <p class="setting-hint">
      Solo se puede editar esta sección en {os === "mac" ? "macOS" : os === "windows" ? "Windows" : "Linux"}.
    </p>
  {/if}

  {#if os === "windows"}
    <section>
      <h3>PowerShell</h3>
      <label class="toggle-row">
        <span>Menú de autocompletado (Tab, Tab)</span>
        <input
          type="checkbox"
          checked={appConfigStore.psreadlineMenuComplete}
          disabled={!active}
          onchange={(e) => appConfigStore.setPsReadlineMenuComplete(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">Solo afecta sesiones nuevas de PowerShell/pwsh abiertas desde Termini.</p>
    </section>
  {:else}
    <section>
      <h3>zsh</h3>
      <label class="toggle-row">
        <span>Menú de autocompletado (Tab, Tab)</span>
        <input
          type="checkbox"
          checked={appConfigStore.zshMenuComplete}
          disabled={!active}
          onchange={(e) => appConfigStore.setZshMenuComplete(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">
        Tab muestra las opciones y un segundo Tab abre un menú que se recorre con las flechas. Solo afecta sesiones
        nuevas de zsh; no modifica tu ~/.zshrc.
      </p>
    </section>

    <section>
      <h3>Bash</h3>
      <label class="toggle-row">
        <span>Prompt con colores (usuario, carpeta y rama de git)</span>
        <input
          type="checkbox"
          checked={appConfigStore.bashPromptColors}
          disabled={!active}
          onchange={(e) => appConfigStore.setBashPromptColors(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">
        {os === "mac" ? "También activa colores en ls y oculta el aviso de zsh. " : ""}Solo afecta sesiones nuevas
        de bash; si tu ~/.bashrc define su propio prompt, se usa ese.
      </p>
    </section>
  {/if}
</div>

<style>
  .platform-section.disabled {
    opacity: 0.5;
    pointer-events: none;
  }

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

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }
</style>
