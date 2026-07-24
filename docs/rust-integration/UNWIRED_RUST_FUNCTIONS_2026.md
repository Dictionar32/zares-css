# Unwired Rust Functions — NAPI Exposed but Not Called from TypeScript

**Status:** ✅ AUDIT COMPLETE  
**Tanggal:** 2026-07-24  
**Total Fungsi NAPI Rust:** 225  
**Total Ter-wire ke TypeScript:** 218  
**Unwired:** 7

---

## Ringkasan

Dari 225 fungsi Rust yang di-expose via NAPI, ada **7 fungsi** yang belum dipanggil dari TypeScript/JavaScript. 6 di antaranya sama sekali belum ada TS wrapper-nya, 1 sudah ada deklarasi opsional tapi tidak dipakai.

---

## Daftar Fungsi Unwired

### 1. `clear_name_registries`
- **File Rust:** `native/src/application/id_registry.rs`
- **TS Declaration:** `packages/domain/engine/src/native-bridge.ts:185`
- **Status:** Deklarasi TS ada, tapi tidak ada pemanggil di codebase.
- **Keterangan:** Fungsi untuk membersihkan semua registry nama yang digunakan untuk scoping class names. Mungkin diperlukan saat hot-reload atau testing.

### 2. `clear_parse_cache_napi_inner`
- **File Rust:** `native/src/infrastructure/napi_bridge_parsing.rs`
- **TS Declaration:** Tidak ada
- **Status:** Belum ada TS wrapper sama sekali.
- **Keterangan:** Fungsi untuk membersihkan cache hasil parsing class names. Bisa dipakai untuk invalidate cache saat theme berubah.

### 3. `clear_theme_cache_napi`
- **File Rust:** `native/src/infrastructure/napi_bridge_theme.rs`
- **TS Declaration:** Tidak ada
- **Status:** Belum ada TS wrapper sama sekali.
- **Keterangan:** Fungsi untuk membersihkan cache tema. Mirip dengan `clear_parse_cache_napi_inner` tapi spesifik untuk theme resolution.

### 4. `extract_theme_from_css`
- **File Rust:** `native/src/infrastructure/napi_bridge_theme.rs`
- **TS Declaration:** `packages/domain/core/src/native.ts:67` (opsional)
- **Status:** Ada deklarasi TS tapi tidak dipanggil. Digantikan oleh `extractThemeFromCssClassified`.
- **Keterangan:** Fungsi legacy untuk ekstrak theme dari CSS content. Sudah digantikan versi yang lebih baik.

### 5. `get_watch_system_status`
- **File Rust:** `native/src/infrastructure/napi_bridge_watch.rs`
- **TS Declaration:** Tidak ada
- **Status:** Belum ada TS wrapper sama sekali.
- **Keterangan:** Fungsi untuk mendapatkan status sistem watcher (active handles, events polled, dll.). Bisa dipakai untuk debugging/monitoring.

### 6. `get_week8_optimization_status`
- **File Rust:** `native/src/infrastructure/week8_api.rs`
- **TS Declaration:** Tidak ada
- **Status:** Belum ada TS wrapper sama sekali.
- **Keterangan:** Fungsi untuk mendapatkan status optimasi week 8 (memory stats, recommendations). Bisa dipakai untuk analytics.

### 7. `inspect_cache_stats`
- **File Rust:** `native/src/infrastructure/cache_analytics.rs`
- **TS Declaration:** Tidak ada
- **Status:** Belum ada TS wrapper sama sekali.
- **Keterangan:** Fungsi untuk mendapatkan statistik cache (hit rate, memory usage, trends). Bisa dipakai untuk monitoring performa.

---

## Rekomendasi

### Prioritas Tinggi
1. **`clear_name_registries`** — Sudah ada deklarasi TS, tinggal tambah pemanggil. Berguna untuk testing dan hot-reload.
2. **`clear_parse_cache_napi_inner`** — Berguna untuk invalidate cache saat theme/config berubah.

### Prioritas Medium
3. **`clear_theme_cache_napi`** — Berguna untuk theme switching.
4. **`get_watch_system_status`** — Berguna untuk debugging file watcher.
5. **`inspect_cache_stats`** — Berguna untuk monitoring cache performa di dev mode.

### Prioritas Rendah
6. **`extract_theme_from_css`** — Legacy function, sudah digantikan. Bisa di-remove dari NAPI exposure.
7. **`get_week8_optimization_status`** — Berguna untuk analytics tapi bukan critical path.

---

## Metodologi Audit

1. **Ekstraksi fungsi NAPI:** Mencari semua `#[napi] pub fn` di `native/src/` menggunakan regex.
2. **Pencarian pemanggil TS:** Mencari nama fungsi (snake_case dan camelCase) di seluruh `packages/`, `src/`, `scripts/`, `test/`, `tests/`.
3. **Verifikasi manual:** Setiap kandidat divalidasi manual menggunakan `rg -n` untuk memastikan tidak ada false positive.

---

## File Terkait

- Audit lengkap semua fungsi Rust: `docs/rust-integration/RUST_FUNCTIONS_AUDIT.md`
- Roadmap integrasi Rust: `docs/rust-integration/RUST_FUNCTIONS_IMPLEMENTATION_ROADMAP.md`
- Cache backend upgrade: `docs/rust-integration/CACHE_BACKEND_UPGRADE_2026.md`
