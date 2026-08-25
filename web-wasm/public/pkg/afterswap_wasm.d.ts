/* tslint:disable */
/* eslint-disable */

export class WasmEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Force-close; returns final value (× entry) or NaN.
     */
    close_position(): number;
    /**
     * Build an engine with demo-relevant knobs; everything else defaults.
     */
    constructor(window_len: number, n_states: number, tranche_frac: number, max_arms: number);
    /**
     * Feed one price tick; returns the events as a JSON array string.
     */
    on_tick(price: number): string;
    /**
     * Open a paper position; returns the entry price or NaN.
     */
    open_position(size: number): number;
    /**
     * Full dashboard snapshot as JSON (same shape as the server's SSE).
     */
    snapshot(n_prices: number): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmengine_free: (a: number, b: number) => void;
    readonly wasmengine_close_position: (a: number) => number;
    readonly wasmengine_new: (a: number, b: number, c: number, d: number) => number;
    readonly wasmengine_on_tick: (a: number, b: number) => [number, number];
    readonly wasmengine_open_position: (a: number, b: number) => number;
    readonly wasmengine_snapshot: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
