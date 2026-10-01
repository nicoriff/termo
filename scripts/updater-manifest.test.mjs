import { describe, expect, it } from "vitest";
import { buildManifest, platformsOf } from "./updater-manifest.mjs";

describe("platformsOf", () => {
  it("knows each platform's update file", () => {
    expect(platformsOf("Termo_0.8.0_x64-setup.exe.sig")).toEqual(["windows-x86_64", "windows-x86_64-nsis"]);
    expect(platformsOf("Termo_aarch64.app.tar.gz.sig")).toEqual(["darwin-aarch64", "darwin-aarch64-app"]);
    expect(platformsOf("Termo_x64.app.tar.gz.sig")).toEqual(["darwin-x86_64", "darwin-x86_64-app"]);
    expect(platformsOf("Termo_0.8.0_amd64.AppImage.sig")).toEqual(["linux-x86_64", "linux-x86_64-appimage"]);
  });

  /** A Termo installed from a .deb or a .rpm asks for its own installer
   * first; without these it fell back to the AppImage, which dpkg can't
   * install, and never updated. */
  it("gives each Linux and Windows installer its own key", () => {
    expect(platformsOf("Termo_0.9.1_amd64.deb.sig")).toEqual(["linux-x86_64-deb"]);
    expect(platformsOf("Termo-0.9.1-1.x86_64.rpm.sig")).toEqual(["linux-x86_64-rpm"]);
    expect(platformsOf("Termo_0.9.1_arm64.deb.sig")).toEqual(["linux-aarch64-deb"]);
    expect(platformsOf("Termo_0.8.0_x64_en-US.msi.sig")).toEqual(["windows-x86_64-msi"]);
  });

  it("leaves out what the updater can't install", () => {
    expect(platformsOf("Termo_0.8.0_x64.dmg.sig")).toEqual([]);
    expect(platformsOf("Termo.app.tar.gz.sig")).toEqual([]);
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
        { name: "Termo_aarch64.app.tar.gz.sig", signature: "sig-mac" },
        { name: "Termo_0.8.0_amd64.AppImage.sig", signature: "sig-appimage" },
        { name: "Termo_0.8.0_amd64.deb.sig", signature: "sig-deb" },
      ],
    });
    expect(manifest.version).toBe("0.8.0");
    expect(manifest.pub_date).toBe("2026-10-01T00:00:00Z");
    expect(manifest.platforms["windows-x86_64"]).toEqual({
      signature: "sig-win",
      url: "https://example.com/v0.8.0/Termo_0.8.0_x64-setup.exe",
    });
    expect(manifest.platforms["darwin-aarch64-app"].url).toBe("https://example.com/v0.8.0/Termo_aarch64.app.tar.gz");
    expect(manifest.platforms["linux-x86_64"].url).toBe("https://example.com/v0.8.0/Termo_0.8.0_amd64.AppImage");
    expect(manifest.platforms["linux-x86_64-deb"]).toEqual({
      signature: "sig-deb",
      url: "https://example.com/v0.8.0/Termo_0.8.0_amd64.deb",
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
