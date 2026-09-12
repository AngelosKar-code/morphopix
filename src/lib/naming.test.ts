import { describe, expect, it } from "vitest";
import { previewOutputName } from "./naming";
import {
  builtinTemplates,
  defaultSettings,
  toProcessOptions,
  type BatchSettings,
} from "./settings";

const withSettings = (overrides: Partial<BatchSettings>): BatchSettings => ({
  ...defaultSettings,
  ...overrides,
});

describe("previewOutputName", () => {
  it("changes the extension when converting", () => {
    const settings = withSettings({ convertEnabled: true, format: "webp" });

    expect(previewOutputName("product.jpg", settings)).toBe("product.webp");
  });

  it("writes jpg rather than jpeg", () => {
    const settings = withSettings({ convertEnabled: true, format: "jpeg" });

    expect(previewOutputName("product.png", settings)).toBe("product.jpg");
  });

  it("keeps the source extension when conversion is off", () => {
    const settings = withSettings({ convertEnabled: false });

    expect(previewOutputName("product.PNG", settings)).toBe("product.png");
  });

  it("leaves names untouched while renaming is off", () => {
    const settings = withSettings({
      renameEnabled: false,
      suffix: "_web",
      spaceHandling: "hyphen",
      convertEnabled: true,
      format: "webp",
    });

    expect(previewOutputName("Blue Summer Dress.jpg", settings)).toBe(
      "Blue Summer Dress.webp",
    );
  });

  it("hyphenates spaces and appends the suffix", () => {
    const settings = withSettings({
      renameEnabled: true,
      suffix: "_web",
      spaceHandling: "hyphen",
      convertEnabled: true,
      format: "webp",
    });

    expect(previewOutputName("Blue Summer Dress.jpg", settings)).toBe(
      "Blue-Summer-Dress_web.webp",
    );
  });

  it("collapses runs of whitespace into a single separator", () => {
    const settings = withSettings({ renameEnabled: true, spaceHandling: "hyphen" });

    expect(previewOutputName("spaced    out.jpg", settings)).toBe("spaced-out.webp");
  });

  it("removes spaces and lowercases together", () => {
    const settings = withSettings({
      renameEnabled: true,
      spaceHandling: "remove",
      lowercaseNames: true,
      convertEnabled: true,
      format: "webp",
    });

    expect(previewOutputName("Blue Summer Dress.jpg", settings)).toBe(
      "bluesummerdress.webp",
    );
  });

  it("falls back to a usable name when the rules empty it", () => {
    const settings = withSettings({ renameEnabled: true, spaceHandling: "remove" });

    expect(previewOutputName("   .jpg", settings)).toBe("image.webp");
  });

  it("handles a file with no extension", () => {
    const settings = withSettings({ convertEnabled: false, renameEnabled: false });

    expect(previewOutputName("noextension", settings)).toBe("noextension");
  });
});

describe("built-in templates", () => {
  const byName = (name: string) => {
    const found = builtinTemplates.find((template) => template.name === name);
    if (!found) throw new Error(`missing template: ${name}`);
    return found;
  };

  it("WooCommerce produces the URL-safe filenames its description promises", () => {
    const woo = byName("WooCommerce product");

    expect(previewOutputName("Blue Summer Dress.JPG", woo.settings)).toBe(
      "blue-summer-dress.webp",
    );
  });

  it("WooCommerce caps at a 2048px master without enlarging smaller files", () => {
    const options = toProcessOptions(byName("WooCommerce product").settings);

    expect(options.maxWidth).toBe(2048);
    expect(options.maxHeight).toBe(2048);
    expect(options.allowUpscale).toBe(false);
    expect(options.format).toBe("webp");
  });

  it("Email safe stays on JPEG, since WebP does not render in Outlook", () => {
    const options = toProcessOptions(byName("Email safe").settings);

    expect(options.format).toBe("jpeg");
    expect(options.maxWidth).toBe(600);
  });

  it("every built-in is a complete settings object", () => {
    for (const template of builtinTemplates) {
      expect(Object.keys(template.settings).sort()).toEqual(
        Object.keys(defaultSettings).sort(),
      );
    }
  });
});
