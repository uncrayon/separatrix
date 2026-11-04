// The only place the UI talks to the maths core.
//
// Inside Tauri the calls go to the Rust commands; in a plain browser
// (`npm run dev`) they go to the template-only mock in ./mock.ts.

import { invoke } from "@tauri-apps/api/core";
import type { CompileRequest, CompileResponse, PortraitRequest, PortraitResponse } from "./types";
import { mockCompile, mockPortrait } from "./mock";

/** True when running inside the Tauri webview (the real Rust core is available). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Parse and analyse the user's equations. */
export function compileSystem(req: CompileRequest): Promise<CompileResponse> {
  return isTauri() ? invoke<CompileResponse>("compile_system", { req }) : mockCompile(req);
}

/** Direction field, nullclines, fixed points and trajectories for one view. */
export function computePortrait(req: PortraitRequest): Promise<PortraitResponse> {
  return isTauri() ? invoke<PortraitResponse>("compute_portrait", { req }) : mockPortrait(req);
}

/** What a scheduler build step returns: the request body plus any data the
 * caller wants handed back with the matching response. */
export interface PortraitJob<M> {
  body: Omit<PortraitRequest, "requestId">;
  meta: M;
}

/**
 * Keeps the portrait "live" without flooding the core.
 *
 * - `request()` can be called as often as you like (every pointer move);
 *   work is coalesced to at most one request per animation frame.
 * - At most one request is in flight; if more changes arrive meanwhile, one
 *   follow-up request is sent with the latest state when it returns.
 * - Every request carries an increasing `requestId`; responses older than the
 *   newest one already delivered are dropped.
 */
export class PortraitScheduler<M> {
  private nextId = 1;
  private newestDelivered = 0;
  private inFlight = false;
  private dirty = false;
  private frame = 0;

  /**
   * @param build Reads the current UI state; return null when nothing can be drawn.
   * @param onResult Receives each fresh response, its meta and the round trip in ms.
   * @param onError Receives failures (the next request still goes out normally).
   */
  constructor(
    private readonly build: () => PortraitJob<M> | null,
    private readonly onResult: (res: PortraitResponse, meta: M, roundTripMs: number) => void,
    private readonly onError: (err: unknown) => void,
  ) {}

  /** Ask for a recompute with whatever the state is at the next frame. */
  request(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      void this.flush();
    });
  }

  /** Stop any queued frame (on unmount). */
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
