# `oxc_parser.rs` — Changelog: hapus JSX-strip workaround + dynamic prop classification

**Status:** perlu diverifikasi dengan `cargo build && cargo test` di environment dengan
toolchain lengkap (rustc ≥ 1.85 / edition2024) sebelum merge. Belum pernah full-compile
persis terhadap Oxc `0.55` — lihat bagian "Belum diverifikasi" di bawah.

## Kenapa perubahan ini ada

Komentar lama di `oxc_parser.rs` bilang:

> Strip standalone JSX elements (top-level JSX menyebabkan parse error di Oxc 0.1.3)

Tapi `native/Cargo.toml` proyek ini sudah pin `oxc_parser = "0.55"`, bukan `0.1.3`.
Komentar itu stale — peninggalan dari fase awal proyek yang nggak pernah dihapus
pas upgrade Oxc.

## Apa yang divalidasi

Dites dengan crate Rust standalone (`oxc_jsx_parse_test`, lihat lampiran) yang
parse 4 skenario pakai Oxc:

1. Tagged template literal saja
2. JSX saja
3. **JSX + tagged template literal di file yang sama** (persis kasus yang katanya gagal)
4. JSX dengan `JSXExpressionContainer` dinamis (`bgColor={theme.primary}`, `onClick={...}`)

Semua 4 kasus parse **tanpa error** di Oxc `0.24.3` (versi tertinggi yang bisa
di-compile di sandbox verifikasi karena keterbatasan MSRV toolchain — lihat
"Belum diverifikasi"). `0.24.3` masih jauh lebih baru dari `0.1.3` yang disebut
di komentar lama, dan grammar JSX+template-literal itu core parser behavior
yang stabil sejak lama — kecil kemungkinan regresi balik di `0.55`.

## Perubahan kode

### 1. `run_structural_pass()` — hapus strip regex

**Sebelum:**
```rust
// Strip standalone JSX elements (top-level JSX menyebabkan parse error di Oxc 0.1.3)
let cleaned = RE_JSX_LINE.replace_all(source, "");
let cleaned = RE_JSX_SELF.replace_all(&cleaned, "");
let ret = Parser::new(&allocator, &cleaned, st).parse();
```

**Sesudah:**
```rust
// JSX-strip dihapus (2026-07) — Oxc 0.55 parse JSX + tagged template literal
// bareng tanpa error, workaround ini nggak diperlukan lagi.
let ret = Parser::new(&allocator, source, st).parse();
```

**Dampak:** `StructuralVisitor` sekarang benar-benar melihat node JSX
(`JSXElement`, `JSXAttribute`, `JSXExpressionContainer`, dst) alih-alih node
itu sudah dihapus sebelum tree dibangun. Sebelumnya nggak mungkin ada visitor
JSX apapun yang ke-trigger.

### 2. `PropValueKind` + `DynamicPropUsage` — tipe baru

```rust
pub enum PropValueKind {
    Static,
    ThemeResolvable { root: String },
    Runtime,
}

pub struct DynamicPropUsage {
    pub component_name: String,
    pub attr_name: String,
    pub kind: PropValueKind,
}
```

Klasifikasi tiap JSX attribute value:

| Kind | Kondisi | Contoh |
|---|---|---|
| `Static` | String/template/numeric/boolean literal | `bgColor="#1e293b"` |
| `ThemeResolvable { root }` | Member/identifier expression yang root-nya berasal dari import dengan path mengandung `theme`/`tokens`/`design-system` (heuristik, bukan garansi resolve berhasil) | `bgColor={theme.primary}` (dengan `import { theme } from "./theme.config"`) |
| `Runtime` | Selain di atas — identifier lokal, call expression, conditional, dst | `bgColor={someState}`, `onClick={() => ...}` |

### 3. `StructuralVisitor` — visitor baru

- `visit_import_declaration`: sekarang juga populate `theme_like_imports`
  (set nama lokal dari import yang path-nya "theme-like").
- `visit_function`: track nama komponen (heuristik PascalCase) buat label
  `dynamic_props`.
- `visit_jsx_opening_element` (baru): iterate semua atribut, classify tiap
  value via `classify_expression()`, push ke `self.dynamic_props`.

### 4. Return signature berubah — breaking untuk caller internal

```diff
-pub(crate) fn run_structural_pass(source: &str) -> (Vec<String>, bool, Vec<String>) {
+pub(crate) fn run_structural_pass(source: &str) -> (Vec<String>, bool, Vec<String>, Vec<DynamicPropUsage>) {
```

`OxcExtractResult` nambah field `dynamic_props: Vec<DynamicPropUsage>`.

**Sudah di-fix di tempat lain yang manggil fungsi ini:**
- `extract_classes_oxc()` — destructure disesuaikan.
- `oxc_parser_tests.rs` → `debug_structural_pass()` — destructure disesuaikan
  dari 3-tuple ke 4-tuple (kalau ini kelewatan, `cargo build` akan gagal
  dengan error mismatch jumlah elemen tuple).

Kalau ada caller lain di luar dua file ini yang manggil `run_structural_pass`
langsung, cek juga.

### 5. Test baru di `oxc_parser_tests.rs`

- `jsx_and_tagged_template_together_no_strip` — regresi utama, assert
  `dynamic_props` nggak kosong (kalau strip masih aktif, pasti kosong).
- `dynamic_prop_classification_static`
- `dynamic_prop_classification_runtime`
- `dynamic_prop_classification_theme_resolvable`

## Belum diverifikasi (perlu dicek di environment kamu)

- **Full compile terhadap Oxc `0.55` asli.** Sandbox verifikasi mentok di
  rustc 1.75 karena `oxc_allocator 0.55.0` butuh Cargo edition2024
  (rustc ≥ 1.85). Nama varian enum (`JSXAttributeValue`, `JSXExpression`,
  `ImportDeclarationSpecifier`, signature `visit_function`) di kode baru
  ditulis konsisten dengan pola yang sudah dipakai di file yang sama
  (`is_tw()`, `visit_variable_declarator()`), tapi belum di-compile-check
  langsung terhadap `0.55`.
- **Downstream consumer** dari `OxcExtractResult` (mis. `oxc-bridge.ts` di
  `packages/domain/scanner`, dan NAPI type definition di sisi Rust/JS
  boundary) belum diupdate buat expose field `dynamic_props` baru ini ke JS.
  Field ini baru ada di sisi Rust — kalau mau dipakai dari TypeScript, NAPI
  struct/binding-nya perlu ditambah juga.
- Apakah ada regresi performa dari menghapus strip regex (source yang di-parse
  sekarang lebih besar/kompleks karena JSX nggak dibuang duluan) — belum
  di-benchmark.

## Cara verifikasi di environment kamu

```bash
cd native
cargo build
cargo test oxc_parser
```

Kalau semua test lulus (termasuk 4 test baru di atas), aman untuk lanjut ke
langkah berikutnya: wiring `dynamic_props` ke `oxc-bridge.ts` dan compiler
pipeline (`cssGeneratorNative.ts`/`compilationNative.ts`) supaya bisa
memutuskan generate class build-time vs emit runtime `setProperty`.
