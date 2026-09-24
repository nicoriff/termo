import { describe, expect, it } from "vitest";
import { breadcrumbs, parentPath, samePath, typeAheadIndex } from "./folder-nav";

describe("parentPath", () => {
  it("walks Windows paths up to the drive, then stops", () => {
    expect(parentPath("E:\\Proyectos\\Termini")).toBe("E:\\Proyectos");
    expect(parentPath("E:\\Proyectos")).toBe("E:\\");
    expect(parentPath("E:\\")).toBeNull();
    expect(parentPath("E:")).toBeNull();
  });

  it("walks POSIX paths up to the root, then stops", () => {
    expect(parentPath("/home/nico/code")).toBe("/home/nico");
    expect(parentPath("/home")).toBe("/");
    expect(parentPath("/")).toBeNull();
  });

  it("ignores a trailing separator", () => {
    expect(parentPath("E:\\Proyectos\\")).toBe("E:\\");
    expect(parentPath("/home/nico/")).toBe("/home");
  });
});

describe("samePath", () => {
  it("treats Windows paths case- and separator-insensitively", () => {
    expect(samePath("E:\\Proyectos\\Termini", "e:/proyectos/termini/")).toBe(true);
    expect(samePath("E:\\", "e:")).toBe(true);
  });

  it("keeps POSIX paths exact", () => {
    expect(samePath("/home/Nico", "/home/nico")).toBe(false);
    expect(samePath("/home/nico/", "/home/nico")).toBe(true);
  });
});

describe("breadcrumbs", () => {
  it("names each level from the drive down", () => {
    expect(breadcrumbs("E:\\Proyectos\\Termini")).toEqual([
      { label: "E:", path: "E:\\" },
      { label: "Proyectos", path: "E:\\Proyectos" },
      { label: "Termini", path: "E:\\Proyectos\\Termini" },
    ]);
  });

  it("starts POSIX paths at /", () => {
    expect(breadcrumbs("/home/nico").map((crumb) => crumb.label)).toEqual(["/", "home", "nico"]);
  });
});

describe("typeAheadIndex", () => {
  const names = ["Apps", "Archive", "build", "src", "static"];

  it("steps through names sharing a first letter on repeated presses", () => {
    expect(typeAheadIndex(names, "s", 0)).toBe(3);
    expect(typeAheadIndex(names, "s", 3)).toBe(4);
    expect(typeAheadIndex(names, "s", 4)).toBe(3);
  });

  it("refines in place as more letters are typed", () => {
    expect(typeAheadIndex(names, "st", 3)).toBe(4);
    expect(typeAheadIndex(names, "ar", 0)).toBe(1);
  });

  it("finds nothing for a prefix no name has", () => {
    expect(typeAheadIndex(names, "z", 0)).toBeNull();
    expect(typeAheadIndex([], "a", 0)).toBeNull();
  });
});
