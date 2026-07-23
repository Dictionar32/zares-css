//! Test-only module extracted from oxc_parser.rs

use crate::oxc_parser::{extract_classes_oxc, extract_classes_regex, run_structural_pass, PropValueKind};

#[cfg(test)]
mod tests {
    use crate::oxc_parser::*;

    #[test]
    fn tagged_template() {
        let r = extract_classes_oxc(
            "const Button = tw.button`bg-blue-500 text-white px-4`",
            "Button.tsx",
        );
        assert!(
            r.classes.contains(&"bg-blue-500".to_string()),
            "{:?}",
            r.classes
        );
        assert!(r.classes.contains(&"px-4".to_string()));
        assert!(r.has_tw_usage);
        assert!(r.component_names.contains(&"Button".to_string()));
    }

    #[test]
    fn object_config() {
        let r = extract_classes_oxc(
            r#"const C = tw.div({ base: "rounded-lg p-4", variants: { s: { sm: "text-sm" } } })"#,
            "C.tsx",
        );
        assert!(
            r.classes.contains(&"rounded-lg".to_string()),
            "{:?}",
            r.classes
        );
        assert!(r.classes.contains(&"text-sm".to_string()));
    }

    #[test]
    fn jsx_classname() {
        let r = extract_classes_oxc(
            r#"<div className="flex items-center hover:bg-gray-100">ok</div>"#,
            "x.tsx",
        );
        assert!(r.classes.contains(&"flex".to_string()), "{:?}", r.classes);
        assert!(r.classes.contains(&"hover:bg-gray-100".to_string()));
    }

    #[test]
    fn use_client() {
        let r = extract_classes_oxc("\"use client\"\nconst X = tw.div`p-4`", "c.tsx");
        assert!(r.has_use_client, "use client tidak terdeteksi");
        assert!(r.classes.contains(&"p-4".to_string()));
    }

    #[test]
    fn imports() {
        let r = extract_classes_oxc(r#"import { tw } from "zares-css""#, "x.ts");
        assert!(r.imports.contains(&"zares-css".to_string()));
    }

    #[test]
    fn mixed_template_and_jsx() {
        let src = [
            "\"use client\"",
            "import { tw } from \"tailwind-styled-v4\"",
            "import React from \"react\"",
            "const Button = tw.button`bg-blue-500 text-white px-4 hover:bg-blue-600`",
            // Trailing `;` required here — without it, this line ending in `})`
            // followed by a line starting with `<div` is ASI-ambiguous: Oxc can
            // parse it as `... }) < div` (a comparison) instead of two separate
            // statements, which fails the whole file's parse. See
            // jsx_and_tagged_template_together_no_strip / debug_parse_error_detail
            // for the isolated repro of this ambiguity.
            "const Card = tw.div({ base: \"rounded-lg shadow-md\", variants: { s: { sm: \"text-sm\" } } });",
            "<div className=\"flex items-center gap-4\">ok</div>",
        ].join("\n");

        let r = extract_classes_oxc(&src, "x.tsx");

        assert!(
            r.classes.contains(&"bg-blue-500".to_string()),
            "template: {:?}",
            r.classes
        );
        assert!(
            r.classes.contains(&"hover:bg-blue-600".to_string()),
            "hover: {:?}",
            r.classes
        );
        assert!(
            r.classes.contains(&"rounded-lg".to_string()),
            "base: {:?}",
            r.classes
        );
        assert!(
            r.classes.contains(&"text-sm".to_string()),
            "variants: {:?}",
            r.classes
        );
        assert!(
            r.classes.contains(&"flex".to_string()),
            "jsx: {:?}",
            r.classes
        );
        assert!(
            r.classes.contains(&"items-center".to_string()),
            "jsx2: {:?}",
            r.classes
        );
        assert!(r.has_use_client, "use client");
        assert!(r.has_tw_usage, "has_tw_usage");
        assert!(
            r.component_names.contains(&"Button".to_string()),
            "names: {:?}",
            r.component_names
        );
        assert!(
            r.component_names.contains(&"Card".to_string()),
            "card: {:?}",
            r.component_names
        );
        assert!(
            r.imports.contains(&"tailwind-styled-v4".to_string()),
            "imports: {:?}",
            r.imports
        );
    }

    #[test]
    fn dynamic_template_no_panic() {
        let r = extract_classes_oxc("const X = tw.div`${cond} flex`", "x.tsx");
        assert!(r.has_tw_usage);
        // Dynamic template — flex tidak diextract (ada ${...})
        // Tapi tidak panic
    }

    #[test]
    fn tw_server_dot() {
        let r = extract_classes_oxc("const X = tw.server.div`bg-white text-gray-900`", "x.tsx");
        assert!(
            r.classes.contains(&"bg-white".to_string()),
            "{:?}",
            r.classes
        );
        assert!(r.classes.contains(&"text-gray-900".to_string()));
    }
}

#[test]
fn debug_mixed_output() {
    let src = [
        "\"use client\"",
        "import { tw } from \"tailwind-styled-v4\"",
        "import React from \"react\"",
        "const Button = tw.button`bg-blue-500 text-white px-4 hover:bg-blue-600`",
        "const Card = tw.div({ base: \"rounded-lg shadow-md\", variants: { s: { sm: \"text-sm\" } } })",
        "<div className=\"flex items-center gap-4\">ok</div>",
    ].join("\n");

    let r = extract_classes_oxc(&src, "x.tsx");
    println!("classes: {:?}", r.classes);
    println!("component_names: {:?}", r.component_names);
    println!("has_use_client: {}", r.has_use_client);
    println!("imports: {:?}", r.imports);

    // Tunjukkan raw regex output
    let raw = extract_classes_regex(&src);
    println!("raw regex classes: {:?}", raw);
}

#[test]
fn debug_structural_pass() {
    let src = [
        "\"use client\"",
        "import { tw } from \"tailwind-styled-v4\"",
        "import React from \"react\"",
        "const Button = tw.button`bg-blue-500`",
        "const Card = tw.div({ base: \"rounded-lg\" })",
        "<div className=\"flex\">ok</div>",
    ]
    .join("\n");

    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use std::path::Path;

    let allocator = Allocator::default();
    let st = SourceType::from_path(Path::new("file.tsx"))
        .unwrap_or_default()
        .with_module(true);
    let ret = Parser::new(&allocator, &src, st).parse();
    println!("parse errors: {}", ret.errors.len());
    println!("stmts: {}", ret.program.body.len());
    println!("directives: {}", ret.program.directives.len());

    // Cek apakah file tsx bisa parse source ini
    let (names, use_client, imports, dynamic_props, parse_errors) =
        run_structural_pass(&src, "x.tsx");
    println!("names: {:?}", names);
    println!("use_client: {}", use_client);
    println!("imports: {:?}", imports);
    println!("dynamic_props: {:?}", dynamic_props);
    println!("parse_errors: {:?}", parse_errors);
}

#[test]
fn debug_parse_error_detail() {
    let src = [
        "\"use client\"",
        "import { tw } from \"tailwind-styled-v4\"",
        "const Button = tw.button`bg-blue-500`",
        "<div className=\"flex\">ok</div>",
    ]
    .join("\n");

    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use std::path::Path;

    // TSX mode
    let alloc1 = Allocator::default();
    let st_tsx = SourceType::from_path(Path::new("file.tsx"))
        .unwrap()
        .with_module(true);
    let ret1 = Parser::new(&alloc1, &src, st_tsx).parse();
    println!(
        "TSX: errors={} stmts={}",
        ret1.errors.len(),
        ret1.program.body.len()
    );

    // Tanpa JSX di akhir
    let src_no_jsx = [
        "\"use client\"",
        "import { tw } from \"tailwind-styled-v4\"",
        "const Button = tw.button`bg-blue-500`",
    ]
    .join("\n");

    let alloc2 = Allocator::default();
    let ret2 = Parser::new(&alloc2, &src_no_jsx, st_tsx).parse();
    println!(
        "TSX no JSX: errors={} stmts={} directives={}",
        ret2.errors.len(),
        ret2.program.body.len(),
        ret2.program.directives.len()
    );

    // Dengan React import
    let src_with_react = [
        "\"use client\"",
        "import React from \"react\"",
        "import { tw } from \"tailwind-styled-v4\"",
        "const Button = tw.button`bg-blue-500`",
        "export default function App() { return <div className=\"flex\">ok</div> }",
    ]
    .join("\n");

    let alloc3 = Allocator::default();
    let ret3 = Parser::new(&alloc3, &src_with_react, st_tsx).parse();
    println!(
        "TSX with func: errors={} stmts={}",
        ret3.errors.len(),
        ret3.program.body.len()
    );
}

#[test]
fn twmerge_multi_arg() {
    let src = "import { twMerge } from 'tailwind-merge'\nexport const cls = twMerge('px-4 py-2', 'bg-blue-500')";
    let r = extract_classes_oxc(src, "x.tsx");
    assert!(
        r.classes.contains(&"px-4".to_string()),
        "px-4 missing: {:?}",
        r.classes
    );
    assert!(
        r.classes.contains(&"bg-blue-500".to_string()),
        "bg-blue-500 missing: {:?}",
        r.classes
    );
}

#[test]
fn cn_multi_arg() {
    let src = "const cls = cn('flex items-center', 'gap-2 p-4')";
    let r = extract_classes_oxc(src, "x.tsx");
    assert!(
        r.classes.contains(&"flex".to_string()),
        "flex missing: {:?}",
        r.classes
    );
    assert!(
        r.classes.contains(&"gap-2".to_string()),
        "gap-2 missing: {:?}",
        r.classes
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Regresi: JSX + tagged template literal di file yang sama, TANPA strip
// regex (RE_JSX_LINE/RE_JSX_SELF dihapus dari run_structural_pass). Kalau
// ada regresi parsing di sini, test ini yang akan gagal duluan.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn jsx_and_tagged_template_together_no_strip() {
    let src = [
        "import { tw } from \"tailwind-styled-v4\"",
        "const Card = tw.div`bg-white rounded-xl p-4`",
        "function DynamicCard({ bgColor, children }) {",
        "  return <Card style={{ background: bgColor }} className=\"flex\">{children}</Card>",
        "}",
    ]
    .join("\n");

    let (names, _use_client, _imports, dynamic_props, parse_errors) =
        run_structural_pass(&src, "x.tsx");

    assert!(
        parse_errors.is_empty(),
        "parse gagal, seharusnya sukses tanpa strip: {:?}",
        parse_errors
    );

    assert!(
        names.contains(&"Card".to_string()),
        "component name tidak terdeteksi: {:?}",
        names
    );
    // Kalau strip masih ada, baris <Card ...> akan terhapus sebelum parsing
    // dan dynamic_props akan selalu kosong walau ada expression container.
    assert!(
        !dynamic_props.is_empty(),
        "dynamic_props kosong — kemungkinan JSX masih ter-strip sebelum parsing"
    );
}

#[test]
fn dynamic_prop_classification_static() {
    let src = r##"function Card() { return <div bgColor="#1e293b" /> }"##;
    let (_, _, _, dynamic_props, _) = run_structural_pass(src, "x.tsx");
    let bg = dynamic_props
        .iter()
        .find(|p| p.attr_name == "bgColor")
        .expect("bgColor tidak ditemukan di dynamic_props");
    assert_eq!(bg.kind, PropValueKind::Static, "{:?}", bg.kind);
}

#[test]
fn dynamic_prop_classification_runtime() {
    // someState adalah local identifier — bukan literal, bukan theme import
    let src = r#"function Card({ someState }) { return <div bgColor={someState} /> }"#;
    let (_, _, _, dynamic_props, _) = run_structural_pass(src, "x.tsx");
    let bg = dynamic_props
        .iter()
        .find(|p| p.attr_name == "bgColor")
        .expect("bgColor tidak ditemukan di dynamic_props");
    assert_eq!(bg.kind, PropValueKind::Runtime, "{:?}", bg.kind);
}

#[test]
fn dynamic_prop_classification_theme_resolvable() {
    // "theme" diimport dari path yang mengandung kata "theme" → heuristik
    // theme_like_imports harus menandai root "theme" sebagai ThemeResolvable
    let src = [
        "import { theme } from \"./theme.config\"",
        "function Card() { return <div bgColor={theme.primary} /> }",
    ]
    .join("\n");

    let (_, _, _, dynamic_props, _) = run_structural_pass(&src, "x.tsx");
    let bg = dynamic_props
        .iter()
        .find(|p| p.attr_name == "bgColor")
        .expect("bgColor tidak ditemukan di dynamic_props");
    assert_eq!(
        bg.kind,
        PropValueKind::ThemeResolvable {
            root: "theme".to_string()
        },
        "{:?}",
        bg.kind
    );
}