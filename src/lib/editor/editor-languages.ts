import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";

/**
 * Markdown as GitHub writes it (tables, ~~struck~~ text, task lists), with
 * its fenced code blocks highlighted in their own language (```ts and the
 * like); language-data's own is plain CommonMark and leaves them plain.
 */
const markdown = LanguageDescription.of({
  name: "Markdown",
  extensions: ["md", "markdown", "mkd", "mdx"],
  load: () =>
    import("@codemirror/lang-markdown").then((m) => m.markdown({ base: m.markdownLanguage, codeLanguages: languages })),
});

/** File names language-data has nothing for, and the language that reads
 * them well enough: by the name's end, lower-cased. */
const ALIASES: [RegExp, string][] = [
  [/\.(jsonc|json5|code-workspace)$|^(tsconfig|jsconfig)\..*json$/, "JSON"],
  [/\.(svelte|astro)$/, "HTML"],
  [/\.zsh$|^\.(zshrc|zprofile|bashrc|bash_profile|profile)$/, "Shell"],
  [/^\.env(\..+)?$|^\.(editorconfig|gitattributes|gitignore|gitmodules|npmrc|dockerignore|prettierignore)$|\.conf$/, "Properties files"],
  [/^cargo\.lock$|\.lock\.toml$/, "TOML"],
];

/** The language a file is read in, by its name; null for plain text. */
export function languageFor(fileName: string): LanguageDescription | null {
  const name = fileName.toLowerCase();
  if (/\.(md|markdown|mkd|mdx)$/.test(name)) return markdown;
  const alias = ALIASES.find(([pattern]) => pattern.test(name))?.[1];
  if (alias) return languages.find((language) => language.name === alias) ?? null;
  return LanguageDescription.matchFilename(languages, fileName);
}
