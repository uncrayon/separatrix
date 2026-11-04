import { invoke } from "@tauri-apps/api/core";
import type {
  CompileRequest,
  CompileResponse,
  ConvertRequest,
  ConvertResponse,
  PortraitRequest,
  PortraitResponse,
} from "./types";
import { mockCompile, mockConvert, mockPortrait } from "./mock";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function compileSystem(req: CompileRequest): Promise<CompileResponse> {
  return isTauri() ? invoke<CompileResponse>("compile_system", { req }) : mockCompile(req);
}

/** Rewrites the equations in the other input format; see `convert_source` in docs/contracts.md. */
export function convertSource(req: ConvertRequest): Promise<ConvertResponse> {
  return isTauri() ? invoke<ConvertResponse>("convert_source", { req }) : mockConvert(req);
}

export function computePortrait(req: PortraitRequest): Promise<PortraitResponse> {
  return isTauri() ? invoke<PortraitResponse>("compute_portrait", { req }) : mockPortrait(req);
}

export interface PortraitJob<M> {
  body: Omit<PortraitRequest, "requestId">;
  meta: M;
}

export class PortraitScheduler<M> {
  private nextId = 1;
  private newestDelivered = 0;
  private inFlight = false;
  private dirty = false;
  private frame = 0;

  constructor(
    private readonly build: () => PortraitJob<M> | null,
    private readonly onResult: (res: PortraitResponse, meta: M, roundTripMs: number) => void,
    private readonly onError: (err: unknown) => void,
  ) {}

  request(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      void this.flush();
    });
  }

  dispose(): void {
    if (this.frame) cancelAnimationFrame(this.frame);
    this.frame = 0;
  }

  private async flush(): Promise<void> {
    if (this.inFlight) {
      this.dirty = true;
      return;
    }
    const job = this.build();
    if (!job) return;
    const requestId = this.nextId++;
    this.inFlight = true;
    const started = performance.now();
    try {
      const res = await computePortrait({ ...job.body, requestId });
      if (res.requestId > this.newestDelivered) {
        this.newestDelivered = res.requestId;
        this.onResult(res, job.meta, performance.now() - started);
      }
    } catch (err) {
      this.onError(err);
    } finally {
      this.inFlight = false;
      if (this.dirty) {
        this.dirty = false;
        this.request();
      }
    }
  }
}
