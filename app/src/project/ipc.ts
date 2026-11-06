// The project-file commands and events, in the desktop app (Rust, see
// docs/contracts.md) or the browser preview (api/mock.ts, files in
// `window.__sepxMock`).
//
// TODO(R-int): these belong in api/client.ts beside the other wrappers; they
// live here until it has them. The command names and `req` shapes are the
// ones lib.rs registers.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "../api/client";
import {
  mockCloseWindow,
  mockLaunchReady,
  mockProjectChooseOpenPath,
  mockProjectChooseSavePath,
  mockProjectRead,
  mockProjectWrite,
  mockRecentProjects,
  mockSetDocumentState,
} from "../api/mock";
import type {
  ChosenSavePath,
  DocumentStateRequest,
  OpenedProject,
  ProjectChooseSaveRequest,
  ProjectOpenRequested,
  ProjectReadRequest,
  ProjectWriteRequest,
  RecentProject,
  SavedProject,
} from "../api/types";

export const EVENT_OPEN_REQUESTED = "project-open-requested";
export const EVENT_CLOSE_REQUESTED = "close-requested";

/** The browser preview's stand-ins for the Rust events (dispatched on `window`, e.g. by the e2e tests). */
export const MOCK_EVENT_OPEN_REQUESTED = "sepx:project-open-requested";
export const MOCK_EVENT_CLOSE_REQUESTED = "sepx:close-requested";

/** An IPC error as one sentence (Rust sends the string itself). */
export function errorText(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}

export const chooseOpenPath = (): Promise<string | null> =>
  isTauri() ? invoke<string | null>("project_choose_open_path") : mockProjectChooseOpenPath();

export const readProject = (req: ProjectReadRequest): Promise<OpenedProject> =>
  isTauri() ? invoke<OpenedProject>("project_read", { req }) : mockProjectRead(req);

export const chooseSavePath = (req: ProjectChooseSaveRequest): Promise<ChosenSavePath | null> =>
  isTauri() ? invoke<ChosenSavePath | null>("project_choose_save_path", { req }) : mockProjectChooseSavePath(req);

export const writeProject = (req: ProjectWriteRequest): Promise<SavedProject> =>
  isTauri() ? invoke<SavedProject>("project_write", { req }) : mockProjectWrite(req);

/** The projects opened or saved lately that still exist, most recent first (granted for `readProject`). */
export const recentProjects = (): Promise<RecentProject[]> =>
  isTauri() ? invoke<RecentProject[]>("recent_projects") : mockRecentProjects();

export const launchReady = (): Promise<string[]> => (isTauri() ? invoke<string[]>("launch_ready") : mockLaunchReady());

export const setDocumentState = (req: DocumentStateRequest): Promise<void> =>
  isTauri() ? invoke<void>("set_document_state", { req }) : mockSetDocumentState(req);

export const closeWindow = (): Promise<void> => (isTauri() ? invoke<void>("close_window") : mockCloseWindow());

/**
 * Listens for "open this file" (a second instance, macOS Open, a drop) and
 * "the window was asked to close while dirty". Resolves once both listeners
 * are attached; the returned function detaches them.
 */
export async function listenDocumentEvents(handlers: {
  onOpenRequested: (path: string) => void;
  onCloseRequested: () => void;
}): Promise<() => void> {
  if (!isTauri()) {
    const open = (e: Event) => {
      const path = (e as CustomEvent<ProjectOpenRequested>).detail?.path;
      if (typeof path === "string") handlers.onOpenRequested(path);
    };
    const close = () => handlers.onCloseRequested();
    window.addEventListener(MOCK_EVENT_OPEN_REQUESTED, open);
    window.addEventListener(MOCK_EVENT_CLOSE_REQUESTED, close);
    return () => {
      window.removeEventListener(MOCK_EVENT_OPEN_REQUESTED, open);
      window.removeEventListener(MOCK_EVENT_CLOSE_REQUESTED, close);
    };
  }
  const offs = await Promise.all([
    listen<ProjectOpenRequested>(EVENT_OPEN_REQUESTED, (e) => handlers.onOpenRequested(e.payload.path)),
    listen(EVENT_CLOSE_REQUESTED, () => handlers.onCloseRequested()),
  ]);
  return () => offs.forEach((off) => off());
}
