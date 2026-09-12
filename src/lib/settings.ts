import type {
  ProcessOptions,
  ResizeFilter,
  SpaceHandling,
  WatermarkPosition,
} from "./api";

export type PerformanceMode = "maximum" | "balanced" | "low";

/**
 * What the user configures. Distinct from `ProcessOptions`, which is what the backend
 * consumes: switching a stage off has to keep its values around so they survive a toggle.
 */
export type BatchSettings = {
  resizeEnabled: boolean;
  maxWidth: number;
  maxHeight: number;
  allowUpscale: boolean;
  filter: ResizeFilter;

  convertEnabled: boolean;
  format: "webp" | "jpeg" | "png";
  quality: number;
  webpLossless: boolean;
  pngMaxCompression: boolean;
  /** Copy files already in the target format instead of recompressing them. */
  passthroughSameFormat: boolean;

  watermarkEnabled: boolean;
  watermarkPath: string;
  watermarkPosition: WatermarkPosition;
  watermarkSizePercent: number;
  watermarkOpacityPercent: number;
  watermarkMarginPercent: number;

  preserveMetadata: boolean;

  renameEnabled: boolean;
  suffix: string;
  spaceHandling: SpaceHandling;
  lowercaseNames: boolean;

  performance: PerformanceMode;
};

export const defaultSettings: BatchSettings = {
  resizeEnabled: true,
  maxWidth: 1920,
  maxHeight: 1920,
  allowUpscale: false,
  filter: "lanczos3",

  convertEnabled: true,
  format: "webp",
  quality: 82,
  webpLossless: false,
  pngMaxCompression: false,
  passthroughSameFormat: false,

  watermarkEnabled: false,
  watermarkPath: "",
  watermarkPosition: "bottom-right",
  watermarkSizePercent: 20,
  watermarkOpacityPercent: 70,
  watermarkMarginPercent: 3,

  preserveMetadata: false,

  renameEnabled: false,
  suffix: "",
  spaceHandling: "hyphen",
  lowercaseNames: false,

  performance: "maximum",
};

/**
 * Ready-made step combinations for the two cases that actually differ in practice.
 *
 * There is no separate "catalog" entry on purpose: WooCommerce generates its own catalog and
 * thumbnail sizes from whatever you upload, so the useful thing to produce is a single
 * master at the largest size the product page will ever need. Shopify does the same through
 * its CDN, which is why one master preset covers both.
 */
export const builtinTemplates: { name: string; hint: string; settings: BatchSettings }[] = [
  {
    name: "WooCommerce product",
    hint: "2048px WebP master · hyphenated lowercase names",
    settings: {
      ...defaultSettings,
      resizeEnabled: true,
      maxWidth: 2048,
      maxHeight: 2048,
      allowUpscale: false,
      filter: "lanczos3",
      convertEnabled: true,
      format: "webp",
      quality: 82,
      // Filenames end up in image URLs and alt text, so make them URL safe.
      renameEnabled: true,
      suffix: "",
      spaceHandling: "hyphen",
      lowercaseNames: true,
    },
  },
  {
    name: "Email safe",
    hint: "600px JPEG · WebP is unsupported in Outlook",
    settings: {
      ...defaultSettings,
      resizeEnabled: true,
      maxWidth: 600,
      maxHeight: 600,
      allowUpscale: false,
      filter: "lanczos3",
      convertEnabled: true,
      format: "jpeg",
      quality: 80,
    },
  },
];

/** Resize off means "no bounding box"; convert off means "write the original format back". */
export function toProcessOptions(settings: BatchSettings): ProcessOptions {
  return {
    maxWidth: settings.resizeEnabled ? settings.maxWidth : null,
    maxHeight: settings.resizeEnabled ? settings.maxHeight : null,
    allowUpscale: settings.allowUpscale,
    filter: settings.filter,
    format: settings.convertEnabled ? settings.format : "keep",
    quality: settings.quality,
    webpLossless: settings.webpLossless,
    pngMaxCompression: settings.pngMaxCompression,
    passthroughSameFormat: settings.passthroughSameFormat,
    // Renaming off means the output keeps the source name, so no suffix and no rewriting.
    suffix: settings.renameEnabled ? settings.suffix : "",
    spaceHandling: settings.renameEnabled ? settings.spaceHandling : "keep",
    lowercaseNames: settings.renameEnabled && settings.lowercaseNames,
    preserveMetadata: settings.preserveMetadata,
    watermark:
      settings.watermarkEnabled && settings.watermarkPath !== ""
        ? {
            path: settings.watermarkPath,
            position: settings.watermarkPosition,
            sizePercent: settings.watermarkSizePercent,
            opacityPercent: settings.watermarkOpacityPercent,
            marginPercent: settings.watermarkMarginPercent,
          }
        : null,
  };
}

/**
 * Each worker holds one decoded frame, so this is the memory dial as much as the speed one.
 * Measured on 300 mixed photos: all 12 cores took 8.8s and peaked at 466MB, 4 cores took
 * 13.7s and peaked at 296MB.
 */
export function threadsFor(mode: PerformanceMode, cores: number): number | null {
  if (mode === "maximum") return null; // backend defaults to every core
  if (mode === "balanced") return Math.max(1, Math.ceil(cores / 2));
  return Math.min(2, cores);
}

/** Guards against a preset file written by an older or hand-edited build. */
export function normalizeSettings(value: unknown): BatchSettings {
  if (typeof value !== "object" || value === null) return { ...defaultSettings };
  return { ...defaultSettings, ...(value as Partial<BatchSettings>) };
}
