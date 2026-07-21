# Dynamic Props — Pengenalan

> Fitur ini juga disebut **"Mode 2: Engine Sadar dari Awal"** di changelog/commit internal. Dokumen ini adalah pengantar lengkapnya — dari kenapa fitur ini ada, cara pakai, sampai batasan yang perlu kamu tau.

## TL;DR

Kalau kamu butuh nilai CSS yang gak diketahui saat build time (warna dari theme runtime, props dari API, dll), tulis aja pakai template placeholder `${...}` di dalam kelas Tailwind bracket:

```tsx
const Card = tw.div({
  base: `rounded-xl shadow-sm p-6 bg-[${bgColor}]`,
})
```

Terus pakai langsung lewat props — **gak perlu `style={}`, gak perlu `setToken()`, gak perlu petunjuk manual apapun**:

```tsx
<Card bgColor={theme.cardBg} />
```

Selesai. Engine yang urus sisanya.

---

## 1. Kenapa Fitur Ini Ada

Sebelum fitur ini, kalau kamu nulis nilai dinamis di dalam `tw.object`, ada tiga kemungkinan nasib:

| Jalur definisi | Perilaku lama |
|---|---|
| `tw.div\`...${x}...\`` | Seluruh komponen **di-skip** dari static compilation, jatuh balik ke runtime |
| `tw(Component)\`...${x}...\`` | Sama — di-skip total |
| `tw.div({ base: \`...${x}...\` })` | **Bug**: `${x}` ke-bake jadi literal class garbage (gak pernah di-skip, gak pernah di-resolve) |

Gak ada satupun jalur yang beneran "sadar" bahwa `${x}` itu nilai dinamis yang bisa di-CSS-Variable-kan otomatis. Developer harus pilih antara nulis fully-static (gak bisa dinamis sama sekali) atau nulis fully-runtime (kehilangan semua keuntungan compile-time).

## 2. Cara Kerja — 3 Tahap

### Tahap 1: Deteksi
Engine (Rust, di `native/src/domain/transform.rs`) scan tiap token kelas di dalam template literal. Kalau bentuknya persis `prefix-[${expr}]` (misal `bg-[${bgColor}]`, `text-[${titleColor}]`), token itu dianggap dinamis — **tanpa perlu hint manual apapun**, cukup dari cara kamu nulis definisinya.

### Tahap 2: Generate CSS Variable
Token dinamis diubah jadi class scoped + CSS custom property:

```css
.tw-Card-bgColor { background-color: var(--Card-bgColor, transparent); }
```

Nama property (`background-color` dari prefix `bg`) di-resolve dari tabel mapping prefix→property yang udah ada di engine (lihat bagian "Batasan" di bawah — ini **bukan** hasil deteksi otomatis dari parser CSS manapun, termasuk Lightning CSS).

### Tahap 3: Auto-generated Props (Approach 3)
Komponen React yang di-generate otomatis nge-destructure prop dengan nama yang sama persis dengan nama variable di dalam `${...}`, terus:
1. Jadiin CSS custom property di `style` root element
2. Di-merge sama `style` yang kamu kasih sendiri (gak ditimpa)
3. Di-`delete` dari props sebelum di-spread ke elemen DOM (jadi gak ada React warning "unknown DOM attribute")

```jsx
// Kira-kira begini hasil generate-nya (disederhanakan)
React.forwardRef(function _Tw_Card(props, ref) {
  var _ds = { "--Card-bgColor": props.bgColor };
  var _st = Object.assign({}, props.style, _ds);
  var _r = Object.assign({}, props);
  delete _r.style; delete _r.bgColor;
  return React.createElement("div", Object.assign({ ref }, _r, {
    className: "tw-Card-bgColor rounded-xl shadow-sm p-6",
    style: _st,
  }));
})
```

## 3. Pemakaian — Definisi ke Pemakaian

### Definisi (`.ts`)

```tsx
// components/ui/Card.ts
import { tw } from "zares-css"

const Card = tw.div({
  base: `rounded-xl shadow-sm p-6 bg-[${bgColor}] text-[${color}]`,
  sub: {
    header: `border-b pb-4 text-lg font-bold text-[${headerColor}]`,
    body: `text-sm text-[${bodyColor}]`,
    footer: `border-t pt-4 flex justify-end gap-2 bg-[${footerBg}]`,
  },
})

export { Card }
```

### Pemakaian (`.tsx`)

```tsx
// app/page.tsx
import { Card } from "@/components/ui"

export default function Page() {
  const theme = useTheme()

  return (
    <Card
      bgColor={theme.cardBg}
      color={theme.cardColor}
      headerColor={theme.titleColor}
      bodyColor={theme.bodyColor}
      footerBg={theme.footerBg}
    >
      <Card.header>Dashboard</Card.header>
      <Card.body>Content here</Card.body>
      <Card.footer>
        <Button>Action</Button>
      </Card.footer>
    </Card>
  )
}
```

Perhatiin: `headerColor`, `bodyColor`, `footerBg` di-set di komponen **paling luar** (`<Card ...>`), padahal dipakainya di `Card.header` / `Card.body` / `Card.footer`. Ini bukan salah ketik — CSS custom property inherit ke bawah lewat DOM tree secara native, jadi selama `Card.header` dkk di-render sebagai children DOM dari `Card` (seperti contoh di atas), nilainya otomatis nyampe.

> ⚠️ Kalau `Card.header` dirender **di luar** `<Card>...</Card>` (bukan sebagai children-nya), inheritance-nya putus dan CSS var gak nyampe. Ini konsekuensi dari cara kerja CSS custom property, bukan keterbatasan buatan engine.

## 4. Kalau Butuh `style={}` Manual Juga

Masih bisa, dan gak konflik:

```tsx
<Card bgColor={theme.cardBg} style={{ transform: "rotate(2deg)" }} />
```

`style` yang kamu kasih di-`Object.assign` bareng CSS var yang di-generate dari props dinamis — dua-duanya jalan bareng, gak saling timpa (kecuali kamu manual nulis key yang sama, misal nulis `style={{ "--Card-bgColor": "red" }}` — itu ketiban `_ds` karena urutan merge-nya `Object.assign({}, props.style, _ds)`, jadi `_ds` menang).

## 5. Prefix yang Sudah Dipetakan

| Prefix | CSS Property | Fallback |
|---|---|---|
| `bg` | `background-color` | `transparent` |
| `text` | `color` ⚠️ (lihat batasan) | `inherit` |
| `border` | `border-color` | `transparent` |
| `fill` | `fill` | `transparent` |
| `stroke` | `stroke` | `transparent` |
| `p`/`px`/`py`/`pt`/`pb`/`pl`/`pr` | `padding*` | `0` |
| `m`/`mx`/`my` | `margin*` | `0` |
| `w` | `width` | `auto` |
| `h` | `height` | `auto` |
| `gap` | `gap` | `0` |
| `rounded` | `border-radius` | `0` |
| `opacity` | `opacity` | `1` |
| `z` | `z-index` | `0` |

Prefix di luar daftar ini tetap menghasilkan CSS Variable, tapi dengan `property: unset` kalau ditulis pakai bentuk `prefix-[${x}]` — pakai bentuk `[property:${x}]` (section 6a) buat hasil yang pasti tanpa perlu nunggu prefix-nya ditambahin ke tabel.

## 6. Batasan yang Perlu Kamu Tau

**a. Ambiguitas prefix `text-` — ✅ ada jalan keluarnya, dan ini sesuai dokumentasi resmi Tailwind sendiri.** Di Tailwind asli, `text-` punya dua arti — warna (`text-red-500`) atau ukuran font (`text-lg`). Untuk token dinamis, `text-[${expr}]` (bentuk polos) **selalu** ditafsirkan sebagai `color`, gak pernah `font-size`.

Tailwind sendiri punya jawaban resmi buat ambiguitas kayak gini — section ["Resolving ambiguities"](https://tailwindcss.com/docs/adding-custom-styles#resolving-ambiguities) di dokumentasi mereka: **CSS data-type hint** pakai parentheses:

```tsx
base: `text-(length:${fontSize}) text-(color:${textColor})`
// → font-size: var(--Comp-fontSize, inherit)
// → color: var(--Comp-textColor, inherit)
```

Engine ngedukung ini persis (plus bentuk bracket versi Tailwind v3 lama, `text-[length:${x}]`, buat back-compat). Selain itu, ada juga opsi **arbitrary property** (`[property:${x}]`) buat CSS property yang Tailwind emang gak punya utility-nya sama sekali:

```tsx
base: `[mask-type:${maskType}]`
```

Bedanya: hint (`prefix-(type:...)`) itu buat utility yang ADA tapi ambigu; arbitrary property (`[prop:...]`) itu buat property yang gak ada utility-nya SAMA SEKALI.

**b. Prefix→property (bentuk `prefix-[${x}]` polos) itu hardcoded manual**, bukan hasil deteksi otomatis dari CSS parser manapun (termasuk Lightning CSS, yang cuma dipakai buat parse file `theme.css`, bukan buat resolve semantik Tailwind). Ini konsisten sama seluruh engine — bahkan Tailwind v4 asli pun tetap pakai tabel mapping eksplisit (`@utility` directive), cuma bentuknya deklaratif. Kalau prefix kamu gak ada di tabel (section 5), pakai bentuk hint (poin a) atau `[property:${x}]` — dua-duanya gak butuh tabel sama sekali.

**c. Nama variable yang sama = satu prop yang sama.** Kalau `${color}` dipakai di `base` DAN di `sub.header`, itu jadi **satu prop** (`color`) yang nge-drive dua CSS Variable sekaligus (`--Card-color` dan `--Card-header-color`). Ini simplifikasi yang disengaja — kalau kamu tulis nama yang sama dua kali, dianggap memang mau nilai yang sama.

**d. Token harus full-match** `prefix-[${expr}]` atau `[property:${expr}]`. Bentuk campuran atau nested expression (`bg-[${a + b}]`, `bg-[${a}px]`, `[font-size:${a}px]`) belum kehandle oleh regex detector saat ini — isi `${...}` harus persis satu nama variable.

**e. Sub-component dynamic props cuma di-set di root element parent**, bukan di masing-masing sub-component-nya sendiri — mengandalkan CSS inheritance (lihat poin di section 3).

## 7. Kaitan dengan Fitur Lain

Fitur ini **melengkapi**, bukan menggantikan:
- `style={}` manual — tetap bisa dipakai bareng (section 4)
- `liveToken()` / `setToken()` — API live-token yang udah ada sebelumnya, cocok buat kasus yang butuh update value di luar siklus render props (misal dari WebSocket/subscription), sedangkan dynamic props ini murni derivasi dari props React biasa.

---

*Dokumen ini nemenin implementasi di `native/src/domain/transform.rs` (deteksi + codegen) dan `native/src/domain/transform_components.rs` (helper codegen `dynamic_style_assign_line` / `delete_dynamic_props_line`). Lihat juga README.md §3.5 untuk versi ringkas.*