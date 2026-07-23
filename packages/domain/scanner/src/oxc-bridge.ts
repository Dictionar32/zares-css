/**
 * tailwind-styled-v4 — Oxc AST bridge untuk scanner.
 *
 * Native-only: Rust Oxc binding is required.
 * No JavaScript fallback — native Rust binding must be available.
 *
 * Mengekspos oxcExtractClasses sebagai pengganti astExtractClasses
 * yang berbasis regex. Lebih akurat karena pakai real AST parser.
 */

import { createRequire } from "node:module"
import path from "node:path"
import { fileURLToPath } from "node:url"

// ESM-compatible __dirname equivalent
function getDirname(): string {
  if (typeof __dirname !== "undefined") {
    return __dirname
  }
  if (typeof import.meta !== "undefined" && import.meta.url) {
    return path.dirname(fileURLToPath(import.meta.url))
  }
  return process.cwd()
}

interface NativeOxcDynamicPropUsage {
  componentName: string
  attrName: string
  kind: "static" | "theme_resolvable" | "runtime"
  themeRoot?: string | null
}

interface NativeOxcBinding {
  oxcExtractClasses?: (
    source: string,
    filename: string
  ) => {
    classes: string[]
    componentNames: string[]
    hasTwUsage: boolean
    hasUseClient: boolean
    imports: string[]
    engine: string
    dynamicProps: NativeOxcDynamicPropUsage[]
    parseErrors: string[]
  }
}

// ─────────────────────────────────────────────────────────────────────────
// Native Oxc Binding - Factory Pattern (no let!)
// ─────────────────────────────────────────────────────────────────────────

const createOxcBindingLoader = () => {
  const _state = { binding: undefined as NativeOxcBinding | null | undefined }

  const getBinding = (): NativeOxcBinding => {
    if (_state.binding !== undefined) {
      if (_state.binding === null) {
        throw new Error(
          "FATAL: Native Oxc binding not found.\n" +
          "This package requires native Rust bindings.\n\n" +
          "Resolution steps:\n" +
          "1. Build the native Rust module: npm run build:rust"
        )
      }
      return _state.binding
    }

    const req = createRequire(import.meta.url)
    const runtimeDir = getDirname()
    const candidates = [
      path.resolve(process.cwd(), "native", "tailwind_styled_parser.node"),
      path.resolve(runtimeDir, "..", "..", "..", "..", "native", "tailwind_styled_parser.node"),
    ]
    for (const c of candidates) {
      try {
        const mod = req(c) as NativeOxcBinding
        if (mod?.oxcExtractClasses) {
          _state.binding = mod
          return mod
        }
      } catch {
        /* next */
      }
    }
    _state.binding = null
    throw new Error(
      "FATAL: Native Oxc binding not found in any candidate path.\n" +
      "This package requires native Rust bindings.\n\n" +
      "Candidates checked:\n" +
      candidates.map((p) => `  - ${p}`).join("\n") +
      "\n\nResolution steps:\n" +
      "1. Build the native Rust module: npm run build:rust"
    )
  }

  return {
    get: getBinding,
    reset: (): void => {
      _state.binding = undefined
    },
  }
}

const oxcBindingLoader = createOxcBindingLoader()

/**
 * Klasifikasi satu JSX attribute value dari komponen dinamis (mis.
 * `bgColor={theme.primary}`), dipakai compiler untuk memutuskan apakah
 * value bisa di-resolve jadi class build-time atau harus fallback ke
 * mekanisme CSS runtime (mis. `element.style.setProperty`).
 *
 * - "static"            → literal (string/template/angka/boolean) → build-time class.
 * - "theme_resolvable"  → member/identifier expression yang root-nya berasal
 *                          dari import "theme-like" (lihat `themeRoot`) — *mungkin*
 *                          bisa di-resolve build-time, compiler masih perlu
 *                          benar-benar load modulnya untuk konfirmasi.
 * - "runtime"           → selain di atas (state lokal, call expression, dst)
 *                          — tidak bisa diketahui saat build.
 */
export interface DynamicPropUsage {
  componentName: string
  attrName: string
  kind: "static" | "theme_resolvable" | "runtime"
  /** Hanya ada kalau `kind === "theme_resolvable"`. */
  themeRoot?: string
}

export interface OxcExtractResult {
  classes: string[]
  componentNames: string[]
  hasTwUsage: boolean
  hasUseClient: boolean
  imports: string[]
  engine: "oxc"
  dynamicProps: DynamicPropUsage[]
  /**
   * Non-kosong kalau Oxc gagal parse `source` secara penuh (mis. ASI hazard:
   * statement tanpa `;` diikuti baris yang diawali `<`). Kalau ini nggak
   * kosong, field lain di atas kemungkinan besar TIDAK LENGKAP untuk file
   * ini — parsing gagal sebelum structural visitor sempat jalan.
   */
  parseErrors: string[]
}

/**
 * Ekstrak kelas Tailwind menggunakan Oxc AST parser (Rust).
 * Lebih akurat dari regex — memahami JSX, TypeScript, template literals.
 *
 * Mengembalikan format yang sama dengan astExtractClasses untuk kompatibilitas.
 */
export function oxcExtractClasses(source: string, filename: string): OxcExtractResult {
  const binding = oxcBindingLoader.get()

  if (!binding.oxcExtractClasses) {
    throw new Error(
      "FATAL: Native binding 'oxcExtractClasses' is required but not available.\n" +
      "This package requires native Rust bindings."
    )
  }

  const r = binding.oxcExtractClasses(source, filename)

  if (r.parseErrors.length > 0) {
    for (const err of r.parseErrors) {
      console.error(
        `[zares-css] oxc parse error — structural extraction (component names, ` +
        `imports, dynamic props) is likely INCOMPLETE for this file: ${err}`
      )
    }
  }

  return {
    classes: r.classes,
    componentNames: r.componentNames,
    hasTwUsage: r.hasTwUsage,
    hasUseClient: r.hasUseClient,
    imports: r.imports,
    engine: "oxc",
    dynamicProps: r.dynamicProps.map((p) => ({
      componentName: p.componentName,
      attrName: p.attrName,
      kind: p.kind,
      ...(p.themeRoot != null ? { themeRoot: p.themeRoot } : {}),
    })),
    parseErrors: r.parseErrors,
  }
}
