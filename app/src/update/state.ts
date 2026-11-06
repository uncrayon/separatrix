import type { UpdateCheck } from "../api/types";

export type UpdateState =
  | { kind: "off" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string }
  | { kind: "installing"; version: string }
  | { kind: "failed"; message: string };

export type UpdateEvent =
  | { type: "check" }
  | { type: "checked"; result: UpdateCheck }
  | { type: "install" }
  | { type: "installFailed"; message: string };

export const CHECK_FAILED = "Couldn't check for updates. You can keep working offline.";

/** The panel only shows once a check starts, so the first state is that check. */
export const initialUpdateState: UpdateState = { kind: "checking" };

export function updateReducer(state: UpdateState, event: UpdateEvent): UpdateState {
  if (state.kind === "installing" && event.type !== "installFailed") return state;
  switch (event.type) {
    case "check":
      return { kind: "checking" };
    case "checked":
      switch (event.result.kind) {
        case "off":
          return { kind: "off" };
        case "upToDate":
          return { kind: "upToDate" };
        case "available":
          return { kind: "available", version: event.result.version };
        case "failed":
          return { kind: "failed", message: CHECK_FAILED };
      }
    case "install":
      return state.kind === "available" ? { kind: "installing", version: state.version } : state;
    case "installFailed":
      return { kind: "failed", message: event.message };
  }
}

export function updateMessage(state: UpdateState): string {
  switch (state.kind) {
    case "checking":
      return "Checking for updates…";
    case "off":
      return "Updates are off in this build";
    case "upToDate":
      return "Separatrix is up to date";
    case "available":
      return `Separatrix ${state.version} is available`;
    case "installing":
      return `Installing Separatrix ${state.version}…`;
    case "failed":
      return state.message;
  }
}
