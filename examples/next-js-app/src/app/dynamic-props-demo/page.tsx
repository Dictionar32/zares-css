"use client"

import { useState } from "react"
import { DynamicCard, DynamicHeading } from "@/components/DynamicCard"

/**
 * Demo penuh — Dynamic Props (§3.5 README / docs/DYNAMIC_PROPS.md)
 *
 * Semua warna Card di bawah ini DIKONTROL LEWAT PROPS RUNTIME (state React biasa),
 * bukan class Tailwind statis. Ganti warnanya lewat color picker — Rust engine
 * sudah generate CSS Variable-nya saat build, jadi tinggal update props aja,
 * gak perlu style={} manual, gak perlu setToken().
 */
export default function DynamicPropsDemoPage() {
  const [bgColor, setBgColor] = useState("#eef2ff") // indigo-50
  const [titleColor, setTitleColor] = useState("#1e1b4b") // indigo-950
  const [bodyColor, setBodyColor] = useState("#4338ca") // indigo-700
  const [footerBg, setFooterBg] = useState("#e0e7ff") // indigo-100
  const [headingSize, setHeadingSize] = useState("28px")
  const [headingColor, setHeadingColor] = useState("#4338ca")

  const presets = [
    { name: "Indigo", bg: "#eef2ff", title: "#1e1b4b", body: "#4338ca", footer: "#e0e7ff" },
    { name: "Rose", bg: "#fff1f2", title: "#4c0519", body: "#be123c", footer: "#ffe4e6" },
    { name: "Emerald", bg: "#ecfdf5", title: "#022c22", body: "#047857", footer: "#d1fae5" },
    { name: "Amber", bg: "#fffbeb", title: "#451a03", body: "#b45309", footer: "#fef3c7" },
  ]

  return (
    <main className="min-h-screen bg-gray-50 p-8">
      <div className="max-w-2xl mx-auto space-y-8">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">Dynamic Props Demo</h1>
          <p className="text-sm text-gray-500 mt-1">
            Semua warna di card ini dikirim lewat props React biasa (bukan `style={}`,
            bukan `setToken()`) — engine yang urus konversi ke CSS Variable saat build.
          </p>
        </div>

        {/* ── Card dengan dynamic props ─────────────────────────────────── */}
        <DynamicCard bgColor={bgColor} titleColor={titleColor} bodyColor={bodyColor} footerBg={footerBg}>
          <DynamicCard.header>
            <span className="font-semibold">Dynamic Card</span>
          </DynamicCard.header>
          <DynamicCard.title>Judul yang warnanya dari props</DynamicCard.title>
          <DynamicCard.body>
            Teks ini pakai <code>{"text-[${bodyColor}]"}</code> — inherit dari
            props <code>bodyColor</code> yang di-set di komponen paling luar
            (<code>&lt;DynamicCard&gt;</code>), bukan di <code>DynamicCard.body</code>
            itu sendiri. CSS custom property yang bikin ini jalan.
          </DynamicCard.body>
          <DynamicCard.footer>
            <span className="text-xs text-gray-500">Footer — bg dinamis juga</span>
          </DynamicCard.footer>
        </DynamicCard>

        {/* ── Preset switcher — ganti value props, gak ada re-mount ────────── */}
        <div className="flex gap-2 flex-wrap">
          {presets.map((p) => (
            <button
              key={p.name}
              onClick={() => {
                setBgColor(p.bg)
                setTitleColor(p.title)
                setBodyColor(p.body)
                setFooterBg(p.footer)
              }}
              className="px-3 py-1.5 rounded-lg border text-sm bg-white hover:bg-gray-100"
            >
              {p.name}
            </button>
          ))}
        </div>

        {/* ── Manual color pickers ─────────────────────────────────────── */}
        <div className="grid grid-cols-2 gap-4 bg-white p-4 rounded-xl border">
          <label className="flex items-center justify-between text-sm">
            bgColor
            <input type="color" value={bgColor} onChange={(e) => setBgColor(e.target.value)} />
          </label>
          <label className="flex items-center justify-between text-sm">
            titleColor
            <input type="color" value={titleColor} onChange={(e) => setTitleColor(e.target.value)} />
          </label>
          <label className="flex items-center justify-between text-sm">
            bodyColor
            <input type="color" value={bodyColor} onChange={(e) => setBodyColor(e.target.value)} />
          </label>
          <label className="flex items-center justify-between text-sm">
            footerBg
            <input type="color" value={footerBg} onChange={(e) => setFooterBg(e.target.value)} />
          </label>
        </div>

        {/* ── Contoh kedua: text- ambiguity hint (§6a) ─────────────────────── */}
        <div className="bg-white p-6 rounded-xl border space-y-3">
          <p className="text-sm text-gray-500">
            Contoh <code>text-(length:...)</code> / <code>text-(color:...)</code> —
            disambiguasi <code>text-</code> antara font-size vs color:
          </p>
          <DynamicHeading headingSize={headingSize} headingColor={headingColor}>
            Heading dinamis ({headingSize})
          </DynamicHeading>
          <div className="flex gap-4">
            <label className="flex items-center gap-2 text-sm">
              Size
              <input
                type="range"
                min="16"
                max="48"
                value={parseInt(headingSize)}
                onChange={(e) => setHeadingSize(`${e.target.value}px`)}
              />
              {headingSize}
            </label>
            <label className="flex items-center gap-2 text-sm">
              Color
              <input type="color" value={headingColor} onChange={(e) => setHeadingColor(e.target.value)} />
            </label>
          </div>
        </div>
      </div>
    </main>
  )
}