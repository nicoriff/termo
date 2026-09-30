import { describe, expect, it } from "vitest";
import { buildManifest, platformOf } from "./updater-manifest.mjs";

describe("platformOf", () => {
  it("knows each platform's update file", () => {
    expect(platformOf("Termo_0.8.0_x64-setup.exe.sig")).toBe("windows-x86_64");
    expect(platformOf("Termo_aarch64.app.tar.gz.sig")).toBe("darwin-aarch64");
    expect(platformOf("Termo_x64.app.tar.gz.sig")).toBe("darwin-x86_64");
    expect(platformOf("Termo_0.8.0_amd64.AppImage.sig")).toBe("linux-x86_64");
  });

  it("leaves out what the updater can't install", () => {
    expect(platformOf("Termo_0.8.0_x64_en-US.msi.sig")).toBeNull();
    expect(platformOf("Termo_0.8.0_amd64.deb.sig")).toBeNull();
    expect(platformOf("Termo.app.tar.gz.sig")).toBeNull();
  });
});

describe("buildManifest", () => {
  const base = {
    version: "0.8.0",
    notes: "notes",
    pubDate: "2026-10-01T00:00:00Z",
    download: (name) => `https://example.com/v0.8.0/${name}`,
  };

  it("points each platform at its file, with its signature", () => {
    const manifest = buildManifest({
      ...base,
      signatures: [
        { name: "Termo_0.8.0_x64-setup.exe.sig", signature: "sig-win\n" },
        { name: "Termo_0.8.0_x64_en-US.msi.sig", signature: "sig-msi" },
        { name: "Termo_aarch64.app.tar.gz.sig", signature: "sig-mac" },
      ],
    });
    expect(manifest.version).toBe("0.8.0");
    expect(manifest.pub_date).toBe("2026-10-01T00:00:00Z");
    expect(manifest.platforms).toEqual({
      "windows-x86_64": { signature: "sig-win", url: "https://example.com/v0.8.0/Termo_0.8.0_x64-setup.exe" },
      "darwin-aarch64": { signature: "sig-mac", url: "https://example.com/v0.8.0/Termo_aarch64.app.tar.gz" },
    });
  });

  it("refuses a release with nothing to install or two files for one platform", () => {
    expect(() => buildManifest({ ...base, signatures: [] })).toThrow();
    expect(() =>
      buildManifest({
        ...base,
        signatures: [
          { name: "A_x64-setup.exe.sig", signature: "a" },
          { name: "B_x64-setup.exe.sig", signature: "b" },
        ],
      }),
    ).toThrow(/two update files/);
  });
});
