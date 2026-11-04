// IPC contract between the webview and the Rust core. See docs/contracts.md.
//
// Every type that crosses IPC is generated from app/src-tauri/src/ipc.rs (and
// the maths crates' enums it reuses) into ipc.generated.ts by ts-rs; this
// module only re-exports them, so the UI compiles against exactly what Rust
// sends. To change one, edit ipc.rs and run
// `UPDATE_IPC_BINDINGS=1 cargo test -p separatrix typescript_bindings`; CI
// fails while ipc.generated.ts is out of date.

export type {
  Arrow,
  AxisRef,
  Classification,
  CompileRequest,
  CompileResponse,
  ConvertRequest,
  ConvertResponse,
  ConvertedLineOut,
  Diagnostic,
  FixedPointOut,
  InputFormat,
  Method,
  NullclineOut,
  ParamInfo,
  PortraitRequest,
  PortraitResponse,
  Severity,
  SolverOptions,
  StateInfo,
  Stats,
  Status,
  SystemKind,
  TrajectoryOut,
  TrajectoryRequest,
} from "./ipc.generated";
