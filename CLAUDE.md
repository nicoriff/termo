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

## Publicar una versión (actualizaciones)

"Buscar actualizaciones" (Acerca de / menú) lee `latest.json` del último release de `github.com/nicoriff/termo-releases` (público) e instala solo builds firmados con la clave cuya parte pública está en `plugins.updater.pubkey` de `tauri.conf.json`.

1. Agregar la entrada de la versión nueva, **en es y en**, arriba de todo en `src/lib/app/changelog.json`. Es lo que ve el usuario al actualizar; el release no sale sin ella (`scripts/release-notes.mjs` falla).
2. Commitear, y `npm version minor` (o `patch`).
3. `git push origin main --tags`. El tag dispara `.github/workflows/release.yml`: compila Windows, macOS (arm64 e Intel) y Linux, sube todo a un release borrador en `termo-releases`, arma `latest.json` (`scripts/publish-release.mjs`) y lo publica.

La clave privada del updater **no va al repo**: vive en los secrets del repo (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). Si se pierde, las instalaciones existentes ya no aceptan actualizaciones. Un `tauri build` local también la necesita en esas variables de entorno (por `bundle.createUpdaterArtifacts`).

## Entorno de desarrollo (Windows)

Antes de `cargo` o `npm run tauri *`, hacer dot-source de `dev-env.ps1`.
