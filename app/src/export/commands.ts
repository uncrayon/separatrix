// The export commands, through Tauri in the app and the mocks in the
// browser preview. (These belong with the other wrappers in api/client.ts;
// they live here until that file gains them.)

import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "../api/client";
import {
  mockClipboardWritePng,
  mockClipboardWriteText,
  mockComputeExportPortrait,
  mockExportContinuationCsv,
  mockExportContinuationLatex,
  mockExportCsv,
  mockExportFigureLatex,
  mockExportFixedPointsLatex,
  mockExportLyapunovCsv,
  mockExportLyapunovLatex,
  mockExportOrbitCsv,
  mockExportOrbitLatex,
  mockExportSystemLatex,
  mockSaveCsv,
  mockSaveContinuationLatex,
  mockSaveCsvFile,
  mockSaveOrbitLatex,
  mockSaveFigureLatex,
  mockSavePngFile,
  mockSaveTextFile,
} from "../api/mock";
import type {
  ContinuationExportRequest,
  ContinuationFigureRequest,
  CsvFileOut,
  CsvRequest,
  CsvResponse,
  ExportPortraitRequest,
  FigureLatexRequest,
  FigureLatexResponse,
  FixedPointsLatexRequest,
  LyapunovExportRequest,
  OrbitExportRequest,
  OrbitFigureRequest,
  PortraitResponse,
  SavedFiles,
  SystemLatexRequest,
  SystemLatexResponse,
  TextExtension,
} from "../api/types";

export function computeExportPortrait(req: ExportPortraitRequest): Promise<PortraitResponse> {
  return isTauri() ? invoke<PortraitResponse>("compute_export_portrait", { req }) : mockComputeExportPortrait(req);
}

export function exportFigureLatex(req: FigureLatexRequest): Promise<FigureLatexResponse> {
  return isTauri() ? invoke<FigureLatexResponse>("export_figure_latex", { req }) : mockExportFigureLatex(req);
}

export function saveFigureLatex(figure: FigureLatexRequest, suggestedName: string): Promise<SavedFiles | null> {
  const req = { figure, suggestedName };
  return isTauri() ? invoke<SavedFiles | null>("save_figure_latex", { req }) : mockSaveFigureLatex(req);
}

export function exportSystemLatex(req: SystemLatexRequest): Promise<SystemLatexResponse> {
  return isTauri() ? invoke<SystemLatexResponse>("export_system_latex", { req }) : mockExportSystemLatex(req);
}

export function exportFixedPointsLatex(req: FixedPointsLatexRequest): Promise<string> {
  return isTauri() ? invoke<string>("export_fixed_points_latex", { req }) : mockExportFixedPointsLatex(req);
}

export function exportCsv(req: CsvRequest): Promise<CsvResponse> {
  return isTauri() ? invoke<CsvResponse>("export_csv", { req }) : mockExportCsv(req);
}

export function saveCsv(csv: CsvRequest, suggestedName: string): Promise<SavedFiles | null> {
  const req = { csv, suggestedName };
  return isTauri() ? invoke<SavedFiles | null>("save_csv", { req }) : mockSaveCsv(req);
}

export function exportLyapunovLatex(req: LyapunovExportRequest): Promise<string> {
  return isTauri() ? invoke<string>("export_lyapunov_latex", { req }) : mockExportLyapunovLatex(req);
}

export function exportLyapunovCsv(req: LyapunovExportRequest): Promise<CsvFileOut> {
  return isTauri() ? invoke<CsvFileOut>("export_lyapunov_csv", { req }) : mockExportLyapunovCsv(req);
}

export function exportOrbitLatex(req: OrbitFigureRequest): Promise<FigureLatexResponse> {
  return isTauri() ? invoke<FigureLatexResponse>("export_orbit_latex", { req }) : mockExportOrbitLatex(req);
}

export function saveOrbitLatex(figure: OrbitFigureRequest, suggestedName: string): Promise<SavedFiles | null> {
  const req = { figure, suggestedName };
  return isTauri() ? invoke<SavedFiles | null>("save_orbit_latex", { req }) : mockSaveOrbitLatex(req);
}

export function exportOrbitCsv(req: OrbitExportRequest): Promise<CsvFileOut> {
  return isTauri() ? invoke<CsvFileOut>("export_orbit_csv", { req }) : mockExportOrbitCsv(req);
}

export function exportContinuationLatex(req: ContinuationFigureRequest): Promise<FigureLatexResponse> {
  return isTauri()
    ? invoke<FigureLatexResponse>("export_continuation_latex", { req })
    : mockExportContinuationLatex(req);
}

export function saveContinuationLatex(figure: ContinuationFigureRequest, suggestedName: string): Promise<SavedFiles | null> {
  const req = { figure, suggestedName };
  return isTauri() ? invoke<SavedFiles | null>("save_continuation_latex", { req }) : mockSaveContinuationLatex(req);
}

export function exportContinuationCsv(req: ContinuationExportRequest): Promise<CsvFileOut> {
  return isTauri() ? invoke<CsvFileOut>("export_continuation_csv", { req }) : mockExportContinuationCsv(req);
}

export function saveCsvFile(file: CsvFileOut, title: string, suggestedName: string): Promise<SavedFiles | null> {
  const req = { file, title, suggestedName };
  return isTauri() ? invoke<SavedFiles | null>("save_csv_file", { req }) : mockSaveCsvFile(req);
}

export function saveTextFile(suggestedName: string, extension: TextExtension, contents: string): Promise<SavedFiles | null> {
  const req = { suggestedName, extension, contents };
  return isTauri() ? invoke<SavedFiles | null>("save_text_file", { req }) : mockSaveTextFile(req);
}

/** Saves PNG bytes; the bytes go as the raw request body, the name in a header. */
export function savePngFile(png: Uint8Array, suggestedName: string): Promise<SavedFiles | null> {
  return isTauri()
    ? invoke<SavedFiles | null>("save_png_file", png, { headers: { "x-sepx-name": encodeURIComponent(suggestedName) } })
    : mockSavePngFile(png, suggestedName);
}

export function clipboardWriteText(text: string): Promise<void> {
  return isTauri() ? invoke<void>("clipboard_write_text", { req: { text } }) : mockClipboardWriteText({ text });
}

export function clipboardWritePng(png: Uint8Array): Promise<void> {
  return isTauri() ? invoke<void>("clipboard_write_png", png) : mockClipboardWritePng(png);
}

/** An IPC error (a string from Rust, or an Error) as a sentence. */
export function errorText(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "Couldn't export: Separatrix got no reason for the failure. Try again; if it keeps happening, restart Separatrix.";
}
