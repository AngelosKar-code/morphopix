import { describe, expect, it } from "vitest";
import {
  commonParentFolder,
  defaultDestination,
  resolveOutputFolder,
  type Destination,
} from "./destination";

const subfolder = (name: string): Destination => ({
  ...defaultDestination,
  mode: "subfolder",
  subfolderName: name,
});

describe("resolveOutputFolder", () => {
  it("nests the output inside the input folder", () => {
    expect(resolveOutputFolder(subfolder("optimized"), "C:\\photos\\autumn")).toBe(
      "C:\\photos\\autumn\\optimized",
    );
  });

  it("keeps posix separators on posix paths", () => {
    expect(resolveOutputFolder(subfolder("web"), "/home/kate/shots")).toBe(
      "/home/kate/shots/web",
    );
  });

  it("does not double up a trailing separator", () => {
    expect(resolveOutputFolder(subfolder("web"), "C:\\photos\\")).toBe("C:\\photos\\web");
  });

  it("falls back to the default name when the field is blank", () => {
    expect(resolveOutputFolder(subfolder("   "), "C:\\photos")).toBe(
      "C:\\photos\\optimized",
    );
  });

  it("cannot resolve a subfolder without an input folder", () => {
    expect(resolveOutputFolder(subfolder("web"), "")).toBe("");
  });

  it("uses the custom folder verbatim when chosen", () => {
    const destination: Destination = {
      mode: "custom",
      subfolderName: "ignored",
      customFolder: "D:\\exports",
    };

    expect(resolveOutputFolder(destination, "C:\\photos")).toBe("D:\\exports");
  });
});

describe("commonParentFolder", () => {
  it("finds the shared parent of dropped files", () => {
    const parent = commonParentFolder([
      "C:\\photos\\a.jpg",
      "C:\\photos\\b.jpg",
      "C:\\photos\\c.jpg",
    ]);

    expect(parent).toBe("C:\\photos");
  });

  it("climbs to the shared level across subfolders", () => {
    const parent = commonParentFolder([
      "C:\\photos\\summer\\a.jpg",
      "C:\\photos\\winter\\b.jpg",
    ]);

    expect(parent).toBe("C:\\photos");
  });

  it("handles a single file", () => {
    expect(commonParentFolder(["/home/kate/shots/a.jpg"])).toBe("/home/kate/shots");
  });

  it("returns nothing for an empty drop", () => {
    expect(commonParentFolder([])).toBe("");
  });
});
