//! TypeScript → JavaScript, in pure Rust via [`oxc`].
//!
//! This is *type stripping* (the esbuild / Deno model), not type checking: type
//! annotations, interfaces, generics, enums, `as` casts etc. are removed/lowered
//! and the resulting JavaScript is handed to boa. No `tsc` (and therefore no
//! type errors) — the trade-off for staying entirely in Rust with no Node/V8.

use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use std::path::Path;

/// Strip the TypeScript in `source` and return runnable JavaScript. On a syntax
/// error, returns the formatted parser diagnostics.
pub fn ts_to_js(source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    // `.ts` (not `.tsx`) — notebook cells aren't JSX, and `<…>` then reads as a
    // type assertion / generic rather than a JSX tag.
    let source_type = SourceType::ts();

    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        let msg = parsed
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(msg);
    }

    let mut program = parsed.program;
    // The transformer needs scope info; build it from semantic analysis.
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();

    let ret = Transformer::new(&allocator, Path::new("cell.ts"), &TransformOptions::default())
        .build_with_scoping(scoping, &mut program);
    if !ret.errors.is_empty() {
        let msg = ret
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(msg);
    }

    Ok(Codegen::new().build(&program).code)
}

#[cfg(test)]
mod tests {
    use super::ts_to_js;

    #[test]
    fn strips_type_annotations() {
        let js = ts_to_js("const n: number = 41; const m = n + 1;").unwrap();
        assert!(js.contains("41"));
        assert!(!js.contains(": number"));
    }

    #[test]
    fn handles_interfaces_and_generics() {
        let src = "interface P { x: number }\nfunction id<T>(v: T): T { return v; }\nconst p: P = { x: id<number>(7) };";
        let js = ts_to_js(src).unwrap();
        assert!(!js.contains("interface"));
        assert!(js.contains("function id"));
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(ts_to_js("const = ;").is_err());
    }
}
