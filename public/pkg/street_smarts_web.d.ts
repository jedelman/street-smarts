/* tslint:disable */
/* eslint-disable */

/**
 * Initialize panic-hook so Rust panics surface as console errors.
 */
export function _start(): void;

/**
 * Evaluate all v0.1 opinions against a neighborhood JSON string.
 * Returns a JSON string of `DisagreementReport`.
 */
export function analyze_neighborhood(neighborhood_json: string): string;

/**
 * List available pattern operators as a JSON array. Each entry has
 * `name`, `description`, and a `source` citation.
 */
export function list_operators(): string;

/**
 * Apply a pattern operator to a parcel inside the given neighborhood JSON.
 * `params_json` is a JSON string of either an object (named params) or
 * an array (vector form). Pass `"null"` or an empty string for defaults.
 * Returns a JSON object: `{ "neighborhood": ..., "trace": ... }`.
 */
export function subdivide_parcel(neighborhood_json: string, parcel_id: string, operator_name: string, params_json: string, seed: bigint): string;

/**
 * Library version string for the UI footer.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly _start: () => void;
    readonly analyze_neighborhood: (a: number, b: number, c: number) => void;
    readonly list_operators: (a: number) => void;
    readonly subdivide_parcel: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: bigint) => void;
    readonly version: (a: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export4: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
