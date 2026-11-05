// Document names: the window title, the top bar's name, the Save dialog's
// suggestion. Pure (no value imports), so node's unit tests import it.

export const APP_NAME = "Separatrix";
export const PROJECT_EXTENSION = "sepx";

/** The last path segment, on either separator. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** `rabbits-vs-sheep.sepx` → `rabbits-vs-sheep`; any other extension is kept. */
export function projectStem(name: string): string {
  const base = baseName(name);
  return base.toLowerCase().endsWith(`.${PROJECT_EXTENSION}`) ? base.slice(0, -PROJECT_EXTENSION.length - 1) : base;
}

/**
 * The document's name: the file's stem once it has a file, otherwise the
 * system's title (a template's, or "Untitled system").
 */
export function documentName(fileName: string | null, systemName: string): string {
  return fileName ? projectStem(fileName) : systemName;
}

/** `"● rabbits-vs-sheep — Separatrix"` while dirty, without the dot when saved. */
export function windowTitle(name: string, dirty: boolean): string {
  return `${dirty ? "● " : ""}${name} — ${APP_NAME}`;
}

/** Lowercase words joined by hyphens, for a file name: `Rabbits vs sheep` → `rabbits-vs-sheep`. */
export function slug(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/**
 * The name the Save dialog offers: the current file's, else the template's
 * id (already a slug), else the system's title as a slug, else "untitled".
 */
export function suggestedProjectName(fileName: string | null, templateId: string | null, systemName: string): string {
  const stem = fileName ? projectStem(fileName) : templateId && templateId !== "blank" ? templateId : slug(systemName);
  return `${stem || "untitled"}.${PROJECT_EXTENSION}`;
}

/** “x.sepx”, with the brand's curly quotes. */
export const quoted = (name: string) => `“${name}”`;
