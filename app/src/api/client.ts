import { invoke } from "@tauri-apps/api/core";
import type {
  CompileRequest,
  CompileResponse,
  ConvertRequest,
  ConvertResponse,
  LyapunovRequest,
  LyapunovResponse,
  OrbitDiagramRequest,
  OrbitDiagramResponse,
  PortraitRequest,
  PortraitResponse,
} from "./types";
import { mockCompile, mockConvert, mockLyapunov, mockOrbitDiagram, mockPortrait } from "./mock";

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

/** The Lyapunov spectrum of one orbit; see `compute_lyapunov` in docs/contracts.md. */
export function computeLyapunov(req: LyapunovRequest): Promise<LyapunovResponse> {
  return isTauri() ? invoke<LyapunovResponse>("compute_lyapunov", { req }) : mockLyapunov(req);
}

/** A map's orbit diagram over one parameter's range; see `compute_orbit_diagram` in docs/contracts.md. */
export function computeOrbitDiagram(req: OrbitDiagramRequest): Promise<OrbitDiagramResponse> {
  return isTauri() ? invoke<OrbitDiagramResponse>("compute_orbit_diagram", { req }) : mockOrbitDiagram(req);
}

/** A request without its `requestId`, which the scheduler numbers, and what the caller keeps with it. */
export interface Job<B, M> {
  body: B;
  meta: M;
}

export type PortraitJob<M> = Job<Omit<PortraitRequest, "requestId">, M>;

/**
 * One request in flight at a time, the newest state asked for next (at most
 * one per frame), and a response older than one already delivered dropped.
 */
export class RequestScheduler<B, R extends { requestId: number }, M> {
  private nextId = 1;
  private newestDelivered = 0;
  private inFlight = false;
  private dirty = false;
  private frame = 0;

  constructor(
    private readonly compute: (req: B & { requestId: number }) => Promise<R>,
    private readonly build: () => Job<B, M> | null,
    private readonly onResult: (res: R, meta: M, roundTripMs: number) => void,
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
      const res = await this.compute({ ...job.body, requestId });
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
