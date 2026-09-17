import { focusNextPending } from "../ai-mode/next-pending";
import { agentSessionsStore } from "../ai-mode/agent-sessions-store.svelte";
import { canRestore, restoreSession, sessionLabel } from "../ai-mode/restore-session";
import { appConfigStore } from "../core/app-config.svelte";
import { resolveDefaultShell } from "../core/default-shell";
import { openPreferencesWindow } from "../core/window-client";
import { t } from "../i18n/i18n.svelte";
import { collectSessions, collectTiles } from "../layout/split-tree";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { tabTitleStore } from "../terminal/tab-title-store.svelte";
import { themeStore } from "../theme/theme-store.svelte";
import type { Command } from "./commands";

/** themeStore's setters never persist on their own — the Preferences sections
 * that own those fields do it. Commands run from the main window, so they have
 * to save themselves or the change is lost on restart. */
function persist(): void {
  void appConfigStore.persist();
}

function focusedTile() {
  const root = workspaceStore.root;
  if (!root || !workspaceStore.focusedTileId) return null;
  return collectTiles(root).find((tile) => tile.id === workspaceStore.focusedTileId) ?? null;
}

/** Every command available right now, already translated and in display
 * order. Called from a `$derived`, so it re-runs when the workspace, the
 * config or the language change. */
export function buildCommands(): Command[] {
  const commands: Command[] = [
    {
      id: "navigation.preferences",
      group: "navigation",
      label: t("commands.openPreferences"),
      run: () => void openPreferencesWindow(),
    },
    {
      id: "navigation.tilePicker",
      group: "navigation",
      label: t("commands.openTilePicker"),
      run: () => workspaceStore.openOverview(),
    },
  ];

  const root = workspaceStore.root;
  for (const loc of root ? collectSessions(root) : []) {
    commands.push({
      id: `navigation.tab.${loc.session.id}`,
      group: "navigation",
      label: t("commands.goToTab", { title: tabTitleStore.titleFor(loc.session.id, loc.session.title) }),
      run: () => workspaceStore.focusSession(loc.session.id),
    });
  }

  commands.push({
    id: "tabs.new",
    group: "tabs",
    label: t("commands.newTab"),
    run: () => workspaceStore.newTabLikeFocused(),
  });

  const tile = focusedTile();
  if (tile) {
    if (tile.activeSessionId) {
      const sessionId = tile.activeSessionId;
      commands.push({
        id: "tabs.close",
        group: "tabs",
        label: t("commands.closeTab"),
        run: () => workspaceStore.closeTab(tile.id, sessionId),
      });
    }
    commands.push(
      {
        id: "tabs.splitVertical",
        group: "tabs",
        label: t("commands.splitVertical"),
        run: () => void resolveDefaultShell().then((shell) => workspaceStore.splitTile(tile.id, "row", shell)),
      },
      {
        id: "tabs.splitHorizontal",
        group: "tabs",
        label: t("commands.splitHorizontal"),
        run: () => void resolveDefaultShell().then((shell) => workspaceStore.splitTile(tile.id, "col", shell)),
      },
      {
        id: "tabs.toggleZoom",
        group: "tabs",
        label: t(workspaceStore.zoomedTileId === tile.id ? "commands.restoreTile" : "commands.zoomTile"),
        run: () => workspaceStore.toggleZoom(tile.id),
      },
    );
  }

  for (const theme of themeStore.themes) {
    commands.push({
      id: `appearance.theme.${theme.name}`,
      group: "appearance",
      label: t("commands.setTheme", { theme: theme.name }),
      run: () => {
        themeStore.setTheme(theme.name);
        persist();
      },
    });
  }

  const languages = [
    { value: "system", name: t("preferences.general.system") },
    { value: "es", name: "Español" },
    { value: "en", name: "English" },
  ] as const;
  for (const language of languages) {
    commands.push({
      id: `appearance.language.${language.value}`,
      group: "appearance",
      label: t("commands.setLanguage", { language: language.name }),
      run: () => appConfigStore.setLanguage(language.value),
    });
  }

  commands.push(
    {
      id: "appearance.blur",
      group: "appearance",
      label: t(appConfigStore.backgroundBlur ? "commands.blurOff" : "commands.blurOn"),
      run: () => appConfigStore.setBackgroundBlur(!appConfigStore.backgroundBlur),
    },
    {
      id: "appearance.highlighting",
      group: "appearance",
      label: t(themeStore.outputHighlightingEnabled ? "commands.highlightOff" : "commands.highlightOn"),
      run: () => {
        themeStore.setOutputHighlighting(!themeStore.outputHighlightingEnabled);
        persist();
      },
    },
    {
      id: "aiMode.toggle",
      group: "aiMode",
      label: t(appConfigStore.aiModeEnabled ? "commands.aiModeOff" : "commands.aiModeOn"),
      run: () => appConfigStore.setAiModeEnabled(!appConfigStore.aiModeEnabled),
    },
  );

  if (appConfigStore.aiModeEnabled) {
    commands.push({
      id: "aiMode.nextPending",
      group: "aiMode",
      label: t("commands.aiNextPending"),
      run: () => focusNextPending(),
    });

    // Same rule as the "+" menu: recent sessions only surface while AI Mode
    // is on, since that's what records them.
    for (const session of agentSessionsStore.sessions) {
      if (!canRestore(session)) continue;
      commands.push({
        id: `aiMode.resume.${session.agentSessionId}`,
        group: "aiMode",
        label: t("commands.resumeSession", { label: sessionLabel(session) }),
        run: () => void restoreSession(session),
      });
    }
  }

  return commands;
}
