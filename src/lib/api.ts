import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type OutputFormat = "webp" | "jpeg" | "png" | "keep";
export type ResizeFilter = "lanczos3" | "catmullrom" | "triangle" | "nearest";
export type SpaceHandling = "keep" | "remove" | "hyphen" | "underscore";

export type WatermarkPosition =
  | "top-left"
  | "top-center"
  | "top-right"
  | "center-left"
  | "center"
  | "center-right"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right";

export type WatermarkOptions = {
  path: string;
  position: WatermarkPosition;
  sizePercent: number;
  opacityPercent: number;
  marginPercent: number;
};

export type ProcessOptions = {
  maxWidth: number | null;
  maxHeight: number | null;
  allowUpscale: boolean;
  filter: ResizeFilter;
  format: OutputFormat;
  quality: number;
  webpLossless: boolean;
  pngMaxCompression: boolean;
  suffix: string;
  passthroughSameFormat: boolean;
  spaceHandling: SpaceHandling;
  lowercaseNames: boolean;
  preserveMetadata: boolean;
  watermark: WatermarkOptions | null;
};

export type ImageEntry = {
  path: string;
  fileName: string;
  sizeBytes: number;
};

export type FileProgress = {
  path: string;
  fileName: string;
  completed: number;
  total: number;
  status: "done" | "error";
  error: string | null;
  originalBytes: number;
  outputBytes: number;
  outputWidth: number;
  outputHeight: number;
};

export type BatchSummary = {
  processed: number;
  failed: number;
  cancelled: boolean;
  totalOriginalBytes: number;
  totalOutputBytes: number;
};

export type PreviewResult = {
  beforeUri: string;
  afterUri: string;
  originalBytes: number;
  outputBytes: number;
  originalWidth: number;
  originalHeight: number;
  outputWidth: number;
  outputHeight: number;
};

export const cpuCount = () => invoke<number>("cpu_count");

export const listImagesInFolder = (
  folder: string,
  recursive: boolean,
  excludeDir: string | null = null,
) => invoke<ImageEntry[]>("list_images_in_folder", { folder, recursive, excludeDir });

export const collectDroppedPaths = (
  paths: string[],
  recursive: boolean,
  excludeDir: string | null = null,
) => invoke<ImageEntry[]>("collect_dropped_paths", { paths, recursive, excludeDir });

export const previewFile = (
  path: string,
  options: ProcessOptions,
  displaySize = 700,
) => invoke<PreviewResult>("preview_file", { path, options, displaySize });

export const processBatch = (
  files: string[],
  outputDir: string,
  options: ProcessOptions,
  maxParallel: number | null = null,
) =>
  invoke<BatchSummary>("process_batch", {
    files,
    outputDir,
    options,
    maxParallel,
  });

export const cancelBatch = () => invoke<void>("cancel_batch");

export const onBatchProgress = (
  handler: (progress: FileProgress) => void,
): Promise<UnlistenFn> =>
  listen<FileProgress>("batch-progress", (event) => handler(event.payload));
