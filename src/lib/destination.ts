/**
 * Where results are written. Writing back over the originals is deliberately not an option:
 * a batch is irreversible and product photos are usually the only copy. The default puts
 * results in a subfolder of the input instead, which is safe and easy to find.
 */
export type DestinationMode = "subfolder" | "custom";

export type Destination = {
  mode: DestinationMode;
  subfolderName: string;
  customFolder: string;
};

export const defaultDestination: Destination = {
  mode: "subfolder",
  subfolderName: "optimized",
  customFolder: "",
};

const SEPARATOR = /[\\/]/;

function joinPath(base: string, child: string): string {
  const separator = base.includes("\\") ? "\\" : "/";
  return `${base.replace(/[\\/]+$/, "")}${separator}${child}`;
}

/** The folder a run would actually write into, or "" when it cannot be resolved yet. */
export function resolveOutputFolder(
  destination: Destination,
  inputFolder: string,
): string {
  if (destination.mode === "custom") return destination.customFolder;
  if (inputFolder === "") return "";

  const name = destination.subfolderName.trim() || defaultDestination.subfolderName;
  return joinPath(inputFolder, name);
}

/**
 * Longest common parent of a set of paths, used as the implied source folder after a drop.
 * Returns "" when the paths share nothing — dropping from two different drives leaves no
 * sensible folder to nest a subfolder inside, and the caller has to ask for one.
 */
export function commonParentFolder(paths: string[]): string {
  if (paths.length === 0) return "";

  const split = paths.map((path) => path.split(SEPARATOR).slice(0, -1));
  const [first, ...rest] = split;

  let shared = first;
  for (const parts of rest) {
    let index = 0;
    while (index < shared.length && index < parts.length && shared[index] === parts[index]) {
      index += 1;
    }
    shared = shared.slice(0, index);
  }

  const separator = paths[0].includes("\\") ? "\\" : "/";
  return shared.join(separator);
}
