import type { BatchSettings } from "./settings";

/**
 * Mirrors `pipeline::output_path_for` / `clean_stem` on the Rust side so the panel can show
 * the resulting filename without a round trip. The two implementations must agree; the
 * cases in `naming.test.ts` are the same ones asserted in the Rust tests.
 */
export function previewOutputName(sourceName: string, settings: BatchSettings): string {
  const lastDot = sourceName.lastIndexOf(".");
  const stem = lastDot > 0 ? sourceName.slice(0, lastDot) : sourceName;
  const sourceExtension = lastDot > 0 ? sourceName.slice(lastDot + 1) : "";

  const extension = settings.convertEnabled
    ? settings.format === "jpeg"
      ? "jpg"
      : settings.format
    : sourceExtension.toLowerCase();

  const withSuffix = settings.renameEnabled ? `${stem}${settings.suffix}` : stem;
  const cleaned = settings.renameEnabled ? cleanStem(withSuffix, settings) : withSuffix;

  return extension === "" ? cleaned : `${cleaned}.${extension}`;
}

function cleanStem(stem: string, settings: BatchSettings): string {
  const words = stem.split(/\s+/).filter((part) => part !== "");

  let spaced: string;
  switch (settings.spaceHandling) {
    case "remove":
      spaced = words.join("");
      break;
    case "hyphen":
      spaced = words.join("-");
      break;
    case "underscore":
      spaced = words.join("_");
      break;
    default:
      spaced = stem;
  }

  const cleaned = settings.lowercaseNames ? spaced.toLowerCase() : spaced;
  return cleaned === "" ? "image" : cleaned;
}
