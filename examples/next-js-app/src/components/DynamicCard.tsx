import React from "react"
import { tw } from "zares-css"

/**
 * DynamicCard — contoh penuh pemakaian Dynamic Props (§3.5 README / docs/DYNAMIC_PROPS.md)
 *
 * Beda dengan `Card` biasa (di Card.tsx) yang semua warnanya statis (compile-time),
 * di sini `bgColor`, `titleColor`, `bodyColor`, `footerBg` DIISI DARI PROPS RUNTIME —
 * misalnya dari theme picker user, API response, dsb — yang gak bisa diketahui
 * saat build time.
 *
 * Cara kerja singkat:
 *   1. `bg-[${bgColor}]` di dalam base/sub → Rust engine detect ini token dinamis
 *   2. Di-compile jadi class scoped + CSS custom property, mis:
 *        .tw-DynamicCard-bgColor { background-color: var(--DynamicCard-bgColor, transparent); }
 *   3. Komponen yang di-generate otomatis nge-destructure prop `bgColor` dari props,
 *      taruh di `style` root sebagai `--DynamicCard-bgColor`, lalu delete dari props
 *      sebelum di-spread ke DOM (gak ada React "unknown attribute" warning)
 *
 * PENTING: prop dynamic (mis. `titleColor`, `bodyColor`, `footerBg`) HARUS di-set
 * di komponen PALING LUAR (<DynamicCard ...>), bukan di masing-masing sub-component-nya.
 * Ini karena CSS custom property inherit ke bawah lewat DOM tree secara native —
 * asal sub-component (Card.header, dst) dirender sebagai children DOM beneran.
 */
export const DynamicCard = tw.article({
  base: `rounded-2xl border border-gray-200 shadow-sm overflow-hidden bg-[${bgColor}]`,
  sub: {
    "header:header": `px-6 pt-5 pb-3 flex items-start justify-between gap-3 border-b border-gray-100`,
    title: `text-base font-semibold leading-snug text-[${titleColor}]`,
    "section:body": `px-6 py-4 text-sm leading-relaxed text-[${bodyColor}]`,
    "footer:footer": `px-6 py-4 flex items-center gap-2 bg-[${footerBg}]`,
  },
})

/**
 * Contoh dengan text- ambiguity hint (§6a docs/DYNAMIC_PROPS.md).
 * `text-` polos = color. Kalau butuh font-size dinamis, WAJIB pakai
 * data-type hint `text-(length:${...})` biar gak ke-interpret jadi color.
 */
export const DynamicHeading = tw.h2({
  base: `font-bold text-(length:${headingSize}) text-(color:${headingColor})`,
})