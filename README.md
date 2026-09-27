# Termo

Terminal de escritorio para Windows/macOS/Linux, construida con
[Tauri 2](https://tauri.app/) + [SvelteKit 5](https://svelte.dev/) +
[xterm.js](https://xtermjs.org/). El backend (Rust) maneja los procesos PTY;
el frontend (Svelte) dibuja las terminales, el layout en splits y la UI.

## Funcionalidades

- **Splits en mosaico**: dividí cualquier panel en vertical/horizontal
  (`SplitView.svelte`, `split-tree.ts`), con resize arrastrando el divisor.
- **Pestañas por panel**: cada panel tiene su propio tabstrip, con
  drag & drop para reordenar o mover una pestaña a otro panel
  (`TileChrome.svelte`).
- **Detección automática de shells**: PowerShell 7, Windows PowerShell,
  cmd, Git Bash y distros de WSL en Windows; `$SHELL` + `/etc/shells` en
  macOS/Linux (`src-tauri/src/pty/shell_detect.rs`). Se elige desde el
  botón "+" de cada panel.
- **Temas**: Dark, Light y Dracula incluidos (`src/lib/theme/presets.ts`),
  con tamaño de fuente, fuente (incluye descarga de Google Fonts) y
  opacidad de fondo configurables.
- **Resaltado de salida**: colorea IPs, URLs, rutas, tamaños, timestamps y
  palabras clave (`error`, `warning`, `done`, etc.) sobre lo que imprime la
  shell, sin tocar lo que el usuario tipea. Es "ANSI-safe": nunca corrompe
  una secuencia de escape aunque llegue partida entre dos lecturas del PTY
  (`src/lib/terminal/output-highlighter.ts`). Se puede desactivar en
  Configuración.
- **Autocompletado tipo menú para PowerShell (opt-in)**: activá "Menú de
  autocompletado (Tab, Tab)" en Configuración → PowerShell para que las
  sesiones nuevas de `pwsh`/`powershell` abran con PSReadLine configurado
  así: `Tab` completa normal, `Tab, Tab` abre un menú navegable (estilo
  oh-my-zsh). No modifica tu `$PROFILE` real, solo aplica a sesiones
  lanzadas desde Termo.
- **Ventana sin bordes nativos**, con su propia titlebar y controles de
  resize.

## Requisitos

- [Node.js](https://nodejs.org/) 18+ y npm
- [Rust](https://www.rust-lang.org/tools/install) (toolchain estable)
- Los [prerrequisitos de Tauri](https://v2.tauri.app/start/prerequisites/)
  para tu sistema operativo (en Windows: WebView2, MSVC Build Tools)

> Si tu setup de Rust/MSVC no es estándar (por ejemplo, herramientas de
> compilación en una unidad distinta a `C:`), mirá `dev-env.ps1` — hacele
> dot-source (`. .\dev-env.ps1`) antes de correr `cargo`/`npm run tauri *`
> para fijar `RUSTUP_HOME`, `CARGO_HOME`, `INCLUDE`/`LIB`/`PATH`. No hace
> falta si tu instalación de Rust y Visual Studio es la default.

## Desarrollo

```bash
npm install
npm run tauri dev
```

Esto levanta el frontend con Vite (`vite dev`, puerto 1420) y compila +
corre el backend de Tauri, con hot-reload para el frontend y recompilación
automática cuando cambia algo en `src-tauri/`.

Otros comandos útiles:

```bash
npm run check       # svelte-check (tipos de Svelte/TS)
npm run tauri build # build de producción, genera el instalador
```

## Estructura del proyecto

```
src/                     Frontend (SvelteKit)
  lib/core/               config, cliente PTY, cliente de ventana, tipos
  lib/layout/             árbol de splits, store del workspace, tabstrip
  lib/terminal/           wrapper de xterm.js, highlighter de salida
  lib/theme/               temas, carga de Google Fonts, tokens de tema
  lib/settings/            panel de configuración
  lib/titlebar/            titlebar custom + bordes de resize
  routes/+page.svelte      punto de entrada de la app

src-tauri/                Backend (Rust)
  src/pty/                 spawn/gestión de procesos PTY, detección de shells
  src/config/              persistencia de configuración (config.json)
  src/commands/            comandos invocables desde el frontend (`invoke`)
```

## Licencia

MIT
