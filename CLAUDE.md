# Termo

## Idiomas (obligatorio)

La app es bilingüe (español / inglés). **Todo texto visible nuevo tiene que existir en los dos idiomas**; nunca se hardcodea en uno solo. Esto incluye etiquetas, `aria-label`, `title`, `placeholder`, notificaciones, errores que se muestran en la UI y títulos de ventana.

- **Frontend:** agregar la clave en `src/lib/i18n/messages/es.ts` y en `src/lib/i18n/messages/en.ts`, y usarla con `t("area.clave")` (de `$lib/i18n/i18n.svelte`). Placeholders con `{nombre}`: `t("preferences.font.installFailed", { error })`. `en.ts` está tipado contra `es.ts`, así que `npm run check` falla si falta o sobra una clave.
- **Lógica pura** (por ejemplo `src/lib/core/shortcuts.ts`, testeada en Node): no importar el store. Devolver un `MessageKey` y traducir en el componente.
- **Rust:** agregar una variante a `Msg` en `src-tauri/src/i18n.rs`, con su traducción para cada `Lang`, y usar `i18n::tr(Msg::...)`. El `match` es exhaustivo, así que no compila si falta un idioma.
- Excepciones: nombres propios y marcas (Termo, Oh My Posh, Claude Code), y los nombres de idioma en el selector ("Español", "English").

El idioma se elige en Preferencias → General (Sistema / Español / English) y se guarda en `AppConfig.language`.

## Versión

La versión de la app se define **solo** en `package.json` (semver: `MAYOR.MENOR.PARCHE`). `tauri.conf.json` la lee de ahí (`"version": "../package.json"`) y se muestra en Acerca de. Para subirla:

```
npm version patch   # arreglos        0.1.0 → 0.1.1
npm version minor   # funciones nuevas 0.1.0 → 0.2.0
```

`npm version` corre `scripts/sync-version.mjs`, que copia la versión a `src-tauri/Cargo.toml` y `Cargo.lock`, y crea el commit y el tag `vX.Y.Z`. Requiere el árbol de trabajo limpio. No editar la versión a mano en `Cargo.toml` ni en `tauri.conf.json`.

## Entorno de desarrollo (Windows)

Antes de `cargo` o `npm run tauri *`, hacer dot-source de `dev-env.ps1`.
