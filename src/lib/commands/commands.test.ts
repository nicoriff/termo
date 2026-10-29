import { describe, expect, it } from "vitest";
import { normalize, scoreMatch } from "../core/fuzzy-match";
import { matchCommands, type Command, type CommandGroup } from "./commands";

function cmd(label: string, group: CommandGroup = "navigation"): Command {
  return { id: label, group, label, run: () => {} };
}

describe("normalize", () => {
  it("drops case and accents", () => {
    expect(normalize("Selección de Tiles")).toBe("seleccion de tiles");
  });
});

describe("scoreMatch", () => {
  it("ranks prefix over word start over contains over subsequence", () => {
    const prefix = scoreMatch("Ir a Preferencias", "ir")!;
    const wordStart = scoreMatch("Ir a Preferencias", "pref")!;
    const contains = scoreMatch("Ir a Preferencias", "eferen")!;
    const subsequence = scoreMatch("Ir a Preferencias", "iap")!;
    expect(prefix).toBeLessThan(wordStart);
    expect(wordStart).toBeLessThan(contains);
    expect(contains).toBeLessThan(subsequence);
  });

  it("matches ignoring accents and case", () => {
    expect(scoreMatch("Selección de tiles", "seleccion")).not.toBeNull();
    expect(scoreMatch("Nueva pestaña", "PESTANA")).not.toBeNull();
  });

  it("returns null when the letters aren't there in order", () => {
    expect(scoreMatch("Nueva pestaña", "zz")).toBeNull();
    expect(scoreMatch("Nueva pestaña", "apu")).toBeNull();
  });
});

describe("matchCommands", () => {
  it("returns everything for an empty query, in registry order", () => {
    const commands = [cmd("Uno"), cmd("Dos"), cmd("Tres")];
    expect(matchCommands(commands, "  ").map((c) => c.label)).toEqual(["Uno", "Dos", "Tres"]);
  });

  it("puts better matches first", () => {
    const commands = [cmd("Cerrar pestaña"), cmd("Nueva pestaña"), cmd("Pestaña siguiente")];
    expect(matchCommands(commands, "pesta").map((c) => c.label)).toEqual([
      "Pestaña siguiente",
      "Cerrar pestaña",
      "Nueva pestaña",
    ]);
  });

  it("keeps group order when scores tie", () => {
    const commands = [cmd("Tema Dark", "appearance"), cmd("Tema: ir a", "navigation")];
    expect(matchCommands(commands, "tema").map((c) => c.group)).toEqual(["navigation", "appearance"]);
  });

  it("drops commands that don't match", () => {
    const commands = [cmd("Ir a Preferencias"), cmd("Nueva pestaña")];
    expect(matchCommands(commands, "preferencias").map((c) => c.label)).toEqual(["Ir a Preferencias"]);
  });
});
