# AGENT.md

Catatan status kerja buat AI agent/dev berikutnya yang lanjutin fitur
"dynamic props → zero(-ish) runtime" di zares-css. Baca ini sebelum nyentuh
`dynamicPropsCompiler.ts`, `dynamicPropWrite.ts`, atau `oxc_parser.rs` bagian
`DynamicPropUsage`/`PropValueKind`.

## Status: fondasi jalan, belum di-wire ke pipeline utama

Yang sudah ada dan **lulus test** (`cargo test oxc_parser` — 17/17):
- `native/src/oxc_parser.rs` — Oxc AST visitor yang classify tiap JSX attribute
  jadi `PropValueKind::Static | ThemeResolvable { root } | Runtime`.
- `native/src/infrastructure/oxc_api.rs` — expose `dynamic_props` +
  `parse_errors` lewat NAPI ke JS/TS.
- `packages/domain/scanner/src/oxc-bridge.ts` — TS-side type `DynamicPropUsage`
  + field `dynamicProps`/`parseErrors` di `OxcExtractResult`.
- `packages/domain/compiler/src/dynamicPropsCompiler.ts` — split logic:
  `static` → build-time class, `runtime` (dan sementara `theme_resolvable`,
  lihat gap #3 di bawah) → minimal-runtime write instruction.
- `packages/domain/runtime-css/src/dynamicPropWrite.ts` — client helper
  `applyDynamicProp()`/`applyDynamicProps()`, pakai `element.style.setProperty()`
  doang. **Bukan** `insertRule` per value, **bukan** Constructable Stylesheet —
  dua pendekatan itu udah dipertimbangkan dan ditolak (SSR butuh fallback
  terpisah, dan rule/class cache bisa numpuk nggak kebatas buat value yang
  terus berubah kayak color picker/slider).

## Yang belum dikerjain — jujur, biar nggak nge-fake progress

1. ✅ **SUDAH di-wire — "Opsi A": extend `scan_file`/`batch_extract_classes`
   di Rust langsung, opt-in via parameter, additive (nggak breaking).**
   (2026-07-23, setelah opsi B/CLI dicoba lalu dicabut atas permintaan user.)

   **Desain:** kedua NAPI fn dapet parameter baru
   `include_dynamic_props: Option<bool>` (default `false` kalau nggak
   dikirim) dan field baru `dynamic_props: Vec<OxcDynamicPropUsage>` di
   struct hasilnya. Kalau `false`/nggak diisi: nol biaya tambahan — jalur
   lama (`extract_tw_classes_from_source`, regex) tetep jalan persis kayak
   sebelumnya, `dynamic_props` cuma `[]` kosong. Kalau `true`: source yang
   udah di-load di memory Rust (nggak baca ulang file) juga di-jalanin lewat
   `oxc_parser::run_structural_pass()` buat classify dynamic props-nya.

   **File yang diubah:**
   - `native/src/application/scanner.rs` — `BatchExtractResult`/
     `ScanFileResult` struct + `batch_extract_classes`/`scan_file` fn, plus
     2 test call site yang perlu disesuaikan (`scan_file(path, None)`).
   - `packages/domain/scanner/src/native-bridge.ts` —
     `batchExtractClassesNative(filePaths, includeDynamicProps = false)`,
     `scanFileNative(filePath, includeDynamicProps = false)`, plus update
     tipe binding interface.
   - `packages/domain/scanner/src/index.ts` — `scanFile(filePath, {
     includeDynamicProps })`; `dynamicProps` di return value **absent**
     (bukan `[]`) kalau opsinya nggak diaktifkan, biar shape hasil scan
     biasa persis sama kayak sebelum fitur ini ada.
   - `packages/domain/scanner/src/schemas.ts` — `ScanFileResultSchema`
     nambah `dynamicProps` optional (zod default strip field yang nggak
     dikenal, jadi ini WAJIB ditambah atau field-nya bakal hilang diam-diam
     pas lewat `parseScanFileResult`).

   **Backward-compat yang udah dicek (bukan cuma diasumsikan):**
   - 2 call site `batchExtractClassesNative(filePaths)` di
     `parallel-scanner.ts` — tetep valid, parameter kedua optional.
   - 2 wrapper duplikat terpisah di `compiler/src/scannerNative.ts` dan
     `compiler/src/analyzer/scannerNative.ts` yang manggil
     `native.batch_extract_classes(filePaths)` langsung — tetep valid
     (NAPI `Option<T>` jadi optional param otomatis), tapi **belum** expose
     opsi `includeDynamicProps` dari jalur ini kalau nanti dibutuhkan di
     situ juga.

   **Belum diverifikasi:** belum sempat `cargo build`/`cargo test`/`pnpm
   build` dijalanin end-to-end di environment lengkap buat perubahan
   scanner.rs ini — sama seperti semua perubahan Rust sebelumnya, WAJIB cek
   dulu (`cargo test` di `native/`) sebelum percaya penuh. Kemungkinan
   ketahuan ada compile error kecil yang saya nggak bisa tangkep tanpa
   toolchain 0.55 lengkap.

   **Belum dikerjain (di luar scope wiring ini):** `compileDynamicProps()`
   masih butuh `literalValues` manual (gap #2) dan treat `theme_resolvable`
   sebagai runtime (gap #3) — jadi walau `dynamicProps` sekarang bisa
   ke-fetch lewat `scanFile`/`batchExtractClassesNative` beneran, buat
   generate CSS statis yang lengkap masih perlu dua gap itu ditutup dulu.

2. **`literalValue` buat kind `static` belum keisi.** Rust classifier di
   `classify_expression()` (`oxc_parser.rs`) baru nentuin "ini static" (lewat
   `Expression::StringLiteral(_) | TemplateLiteral(_) | ...`) tapi belum
   extract teks literalnya ke `DynamicPropUsage`. `dynamicPropsCompiler.ts`
   sekarang expect `literalValues` di-supply manual dari luar (lewat param
   kedua `compileDynamicProps(usages, literalValues)`) — kalau nggak ada,
   entry static di-skip dengan `console.warn`, bukan generate class dengan
   value palsu/kosong. Fix yang bener: tambah field
   `literal_value: Option<String>` di `DynamicPropUsage` (Rust), diisi pas
   `classify_expression` ketemu `StringLiteral`/`TemplateLiteral` tanpa
   interpolasi, terus threading sampai NAPI + TS types.

3. **Gap `theme_resolvable`.** `PropValueKind::ThemeResolvable { root }`
   cuma nyimpen root identifier (`"theme"`), bukan full member-expression
   path (`"primary"` dari `theme.primary`). Tanpa path itu, nggak ada cara
   compiler beneran lookup value di theme object — jadi sementara
   `theme_resolvable` di-treat sama kayak `runtime` di
   `dynamicPropsCompiler.ts` (aman & benar, cuma belum optimal). Fix:
   - Rust: tambah `theme_path: Vec<String>` di `DynamicPropUsage`, diisi di
     `classify_expression`/`root_identifier` dengan collect tiap `.property`
     name pas walk `MemberExpression` chain (sekarang cuma ambil root-nya,
     buang sisanya).
   - Thread `theme_path` lewat `oxc_api.rs` → `oxc-bridge.ts` →
     `DynamicPropUsage` TS type.
   - Di `dynamicPropsCompiler.ts`, implement `resolveThemePath(theme, path)`
     (semacam lodash `_.get`) dan route hasil resolve sukses ke
     `staticClasses`, gagal resolve tetep fallback ke `runtimeWrites`.

4. ⚠️ **SEBAGIAN dikerjain (2026-07-23, atas konfirmasi eksplisit user "iya,
   lanjut kerjain sekarang juga") — API-nya udah bisa nerima source
   location, TAPI belum ada yang otomatis ngisi nilainya dari scan source
   code.** Jangan salah baca ini sebagai "source location udah jalan
   otomatis" — belum. Yang beres cuma separuh: *kemampuan nerima* data
   posisi. *Sumber* data posisinya (scanner nge-extract span dari AST)
   masih belum dikerjain sama sekali (lihat "Belum dikerjain lanjutan" di
   bawah).

   **Desain (bagian yang SUDAH beres):** `compile_to_css` dapet 3 parameter
   baru di akhir: `file: Option<String>`, `line: Option<u32>`, `column:
   Option<u32>` — kalau nggak dikirim (default semua `None`), behavior 100%
   sama kayak sebelum perubahan ini. `compile_to_css_batch` dapet parameter
   `sources: Option<Vec<Option<SourceLocation>>>`, harus sama panjang
   dengan `inputs` kalau disediakan (dicek eksplisit, error kalau nggak
   match), tiap index independen boleh `None`. Caller yang UDAH TAU posisi
   class-nya (misal manual, atau dari sumber lain) bisa langsung pakai ini
   sekarang.

   **File yang diubah:**
   - `native/src/infrastructure/napi_bridge_types.rs` — `SourceLocation`
     ditambah `#[napi(object)]` (tadinya cuma `Serialize/Deserialize`,
     dipakai lewat JSON string di `generate_css`) supaya bisa dipassing
     langsung sebagai NAPI param di `compile_to_css_batch`.
   - `native/src/infrastructure/napi_bridge_css.rs` — `compile_to_css`,
     `compile_to_css_batch` (param baru + populate `rule.source`); **fix
     bug tersembunyi yang ketemu sambil ngerjain ini**: `cache_key` di
     `compile_to_css` sebelumnya cuma berdasar `input`+`minify` — kalau
     nggak disertain `file`/`line`/`column`, dua panggilan dengan `input`
     sama tapi source location beda bakal salah ambil hasil cache dari
     source yang lain (stale source comment di output). Sekarang
     `source_key` ikut masuk cache key.
   - `packages/domain/compiler/src/cssCompilationNative.ts` DAN duplikatnya
     `packages/domain/compiler/src/compiler/cssCompilationNative.ts` (2 file
     terpisah yang isinya mirip tapi sempat divergen — lihat catatan di
     bawah) — `compileToCss`/`compileToCssBatch` nerima parameter opsional
     `source`/`sources` berbentuk `{file, line, column}`.
   - `packages/domain/compiler/src/nativeBridge.ts` — update tipe binding
     interface buat `compile_to_css`/`compile_to_css_batch`.

   **Belum dikerjain lanjutan (supaya beneran end-to-end otomatis):**
   - Scanner/AST layer (`oxc_parser.rs`) belum nge-capture span (Oxc AST
     node punya `.span()`, byte-offset) terus convert ke line:column — mirip
     logic yang sempat ditulis terus DIBUANG pas bikin `format_parse_errors`
     (waktu itu diputuskan nggak perlu buat kebutuhan saat itu; sekarang
     justru dibutuhin lagi buat kasus ini, cek versi lama function itu kalau
     mau referensi).
   - `DynamicPropUsage`/class extraction manapun belum nyertain posisi
     source — jadi walau `compile_to_css` sekarang BISA nerima source, nggak
     ada satupun caller otomatis (scanner → compiler pipeline) yang beneran
     ngisinya. Tetep manual sampai bagian ini dikerjain.
   - Titik integrasi paling masuk akal: scanner nangkep span pas nemuin
     class (baik `className="..."` JSX atribut atau tagged template), thread
     sampai ke titik yang manggil `compileToCss`/`compileToCssBatch`.

   **Temuan sampingan yang belum ditindaklanjuti:** `cssCompilationNative.ts`
   ternyata punya 2 salinan yang SUDAH DIVERGEN sebelum sesi ini — versi di
   `compiler/` subfolder punya fungsi ekstra `generateCss`/`generateCssBatch`
   (wrapper ke NAPI fn `generate_css`/`generate_css_batch` yang beda dari
   `compile_to_css` — nerima `CssRule` JSON utuh langsung, termasuk `source`,
   tanpa perlu parse nama class) yang nggak ada di salinan satunya. Kedua
   salinan sekarang saya update konsisten buat `compileToCss`/
   `compileToCssBatch`, tapi divergensi asalnya (kenapa ada 2 file, kenapa
   `generateCss`/`generateCssBatch` cuma ada di satu) belum diinvestigasi —
   worth dicek apakah salah satu emang seharusnya dihapus/di-reexport ulang
   alih-alih duplikat manual.

   **Belum diverifikasi:** belum sempat `cargo build`/`cargo test`/`pnpm
   build` dijalanin end-to-end buat perubahan ini — WAJIB cek dulu sebelum
   percaya penuh, sama seperti semua perubahan Rust sebelumnya di sesi ini.

## Prinsip yang dipegang sepanjang kerjaan ini (jangan dilanggar pas lanjutin)

- **Jangan klaim "zero runtime" kalau propnya beneran nilai runtime**
  (state, hasil komputasi, API response). Itu keterbatasan matematis, bukan
  keterbatasan implementasi — CSS statis nggak bisa berisi nilai yang belum
  ada saat file itu ditulis. Target realistis: build-time buat yang bisa
  (static + theme-resolvable-yang-berhasil-di-resolve), minimal-runtime
  (`setProperty`, bukan `insertRule`) buat sisanya.
- **Jangan nulis komentar/dokumentasi yang nge-klaim sesuatu udah kepegang
  padahal belum divalidasi.** Ini persis bug yang diperbaiki di awal kerjaan
  ini — komentar lama bilang "Oxc 0.1.3 gagal parse JSX + tagged template",
  padahal proyek udah lama pindah ke Oxc 0.55 dan itu udah nggak masalah;
  komentar basi itu nggak pernah dihapus dan bikin keputusan arsitektur yang
  keliru (strip-JSX-via-regex) bertahan lama tanpa ada yang curiga.
- Kalau ragu API/tipe data yang belum pernah di-compile-check (nama field
  Rust, path module, dst), **jangan nebak field internal** — pakai trait
  method yang udah kebukti jalan (`Display`/`{}` daripada akses field
  spesifik), atau generic function, atau langsung declare butuh verifikasi
  manual di environment yang toolchain-nya lengkap.

## Perubahan lanjutan (2026-07-24) — setelah fondasi dynamic props

### 1. Rust — Cache Backend Upgrade
- `native/src/infrastructure/adapters.rs` — tambah `LazyCacheAdapter` dengan
  timeout-based expiration, `StringKeyedAdaptiveCache` wrapping
  `AdaptiveCache<String, String>`, dan `DistributedCacheAdapter` wrapping
  `RedisDistributedCache`.
- `native/src/infrastructure/cache_backend.rs` — `CacheConfig::Lazy` sekarang
  memakai `LazyCacheAdapter`, `CacheConfig::Adaptive` memakai
  `StringKeyedAdaptiveCache` dengan `max_capacity`, `CacheConfig::Distributed`
  memakai `DistributedCacheAdapter`.
- `native/src/infrastructure/adaptive_cache.rs` — tambah `remove(key: &K) -> bool`
  dan `contains(key: &K) -> bool`.
- `native/src/infrastructure/redis_distributed.rs` — tambah `clear()`, `len()`,
  `is_empty()`.
- `native/src/scan_cache.rs` — hapus `#[allow(dead_code)]` dari
  `priority_score`, `cache_dump`, `cache_load`, dan field `mtime_ms`/`size`
  di `CacheEntry`.

### 2. Rust — Bug Fixes
- `native/src/animation.rs` — ganti `WHITESPACE_RE.split()` menjadi
  `split_whitespace()` di `split_animate_classes`, hapus dependency regex
  yang tidak terpakai, dan fix `generate_id()` agar tidak panic
  dengan `.unwrap_or(0)` fallback.
- `native/src/domain/css_rule.rs` — extract helper
  `nest_media_queries(inner: String) -> String` untuk menghindari duplikasi
  loop fold antara `to_css_string()` dan `to_minified_css()`.
- `native/src/domain/transform.rs` — tambah `has_unparseable_dynamic_expressions()`
  agar template dengan `${` yang tidak bisa di-resolve kini diskip di
  STEP 1 AST path, STEP 1 regex fallback, dan STEP 2 `tw(Component)` wrapper.
- `native/src/watcher_tests.rs` — ganti hardcoded `/tmp` dengan
  `tempfile::TempDir` agar test tidak gagal di environment dengan
  permission restriction.
- `native/src/infrastructure/napi_bridge_css.rs` — hapus duplicate `#[napi]`
  dan fix type mismatch `&Variant` vs `&str` dengan `v.name()`.

### 3. TypeScript — Wire Unwired Functions
7 fungsi Rust yang sebelumnya exposed via NAPI tapi belum dipanggil dari
TypeScript sekarang sudah di-wire:

- `packages/domain/engine/src/native-bridge.ts` — tambah wrapper
  `clearNameRegistries()`, export via `index.ts`.
- `packages/domain/compiler/src/nativeBridge.ts` — tambah interface untuk
  `clearThemeCacheNapi`, `getWatchSystemStatus`, `getWeek8OptimizationStatus`,
  `inspectCacheStats`.
- `packages/domain/compiler/src/nativeBridgeWrappers.ts` — tambah wrapper +
  types:
  - `clear_parse_cache_napi`
  - `clear_theme_cache_napi`
  - `get_watch_system_status` + `WatchSystemStatus`
  - `get_week8_optimization_status` + `Week8OptimizationStatus`
  - `inspect_cache_stats` + `CacheInspectionResult`
- `packages/domain/compiler/src/cache/index.ts` — export
  `clear_parse_cache_napi`, `clear_theme_cache_napi`.
- `packages/domain/compiler/src/watch/index.ts` — export `get_watch_system_status`.
- `packages/domain/compiler/src/analyzer/index.ts` — export
  `get_week8_optimization_status`, `inspect_cache_stats`.
- `packages/domain/core/src/native.ts` — tambah wrapper `extractThemeFromCss()`,
  export via `index.ts`.

### 4. Tests Baru
- `packages/domain/compiler/tests/nativeBridgeWrappers.test.mjs` — test untuk
  5 fungsi newly wired.
- `packages/domain/engine/tests/nativeBridge.test.mjs` — test untuk
  `clearNameRegistries`.
- `packages/domain/core/tests/nativeBridge.test.mjs` — test untuk
  `extractThemeFromCss`.

### 5. Dokumentasi
- `docs/rust-integration/CACHE_BACKEND_UPGRADE_2026.md`
- `docs/rust-integration/UNWIRED_RUST_FUNCTIONS_2026.md`
- `docs/rust-integration/WIRE_UNWIRED_RUST_FUNCTIONS_2026.md`

### 6. Config & Ignore
- `.npmignore` — tambah `kilo.json` dan `.kilo`.
- `.gitignore` — tambah `kilo.json`, `.kilo`, `.kilocode`.
- `.kilocode/mcp.json` — konfigurasi Context7 MCP remote server.
- `kilo.json` — hapus blok MCP config (dipindah ke `.kilocode/mcp.json`).

### 7. Test Fixes
- `native/src/domain/transform.rs` — `transform_source_skips_dynamic_templates`
  kini lulus.
- `native/src/watcher_tests.rs` — `watcher_starts_on_real_dir` kini lulus.
- Full suite: `cargo test --lib` — **635 passed, 0 failed**.

### 8. Context7 MCP
- Remote MCP server `https://mcp.context7.com/mcp` dikonfigurasi di
  `.kilocode/mcp.json` dengan API key.
- `kilo.json` tidak dipakai untuk MCP config lagi di project ini.
- Setelah edit `.kilocode/mcp.json`, restart Kilo/VS Code agar aktif.