//! Size probe: links every grammar so the linker cannot drop one; `sizeprobe LANG` parses an empty file.
fn lang(name: &str) -> Option<tree_sitter::Language> {
    Some(match name {
        #[cfg(feature = "g-rust")]
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        #[cfg(feature = "g-java")]
        "java" => tree_sitter_java::LANGUAGE.into(),
        #[cfg(feature = "g-typescript")]
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        #[cfg(feature = "g-typescript")]
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        #[cfg(feature = "g-bash")]
        "bash" => tree_sitter_bash::LANGUAGE.into(),
        #[cfg(feature = "g-c")]
        "c" => tree_sitter_c::LANGUAGE.into(),
        #[cfg(feature = "g-cpp")]
        "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        #[cfg(feature = "g-c-sharp")]
        "csharp" => tree_sitter_c_sharp::LANGUAGE.into(),
        #[cfg(feature = "g-css")]
        "css" => tree_sitter_css::LANGUAGE.into(),
        #[cfg(feature = "g-cuda")]
        "cuda" => tree_sitter_cuda::LANGUAGE.into(),
        #[cfg(feature = "g-dart")]
        "dart" => tree_sitter_dart::LANGUAGE.into(),
        #[cfg(feature = "g-elixir")]
        "elixir" => tree_sitter_elixir::LANGUAGE.into(),
        #[cfg(feature = "g-glsl")]
        "glsl" => tree_sitter_glsl::LANGUAGE_GLSL.into(),
        #[cfg(feature = "g-go")]
        "go" => tree_sitter_go::LANGUAGE.into(),
        #[cfg(feature = "g-groovy")]
        "groovy" => tree_sitter_groovy::LANGUAGE.into(),
        #[cfg(feature = "g-haskell")]
        "haskell" => tree_sitter_haskell::LANGUAGE.into(),
        #[cfg(feature = "g-hlsl")]
        "hlsl" => tree_sitter_hlsl::LANGUAGE_HLSL.into(),
        #[cfg(feature = "g-html")]
        "html" => tree_sitter_html::LANGUAGE.into(),
        #[cfg(feature = "g-javascript")]
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        #[cfg(feature = "g-json")]
        "json" => tree_sitter_json::LANGUAGE.into(),
        #[cfg(feature = "g-kotlin-ng")]
        "kotlin" => tree_sitter_kotlin_ng::LANGUAGE.into(),
        #[cfg(feature = "g-lua")]
        "lua" => tree_sitter_lua::LANGUAGE.into(),
        #[cfg(feature = "g-objc")]
        "objc" => tree_sitter_objc::LANGUAGE.into(),
        #[cfg(feature = "g-php")]
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        #[cfg(feature = "g-python")]
        "python" => tree_sitter_python::LANGUAGE.into(),
        #[cfg(feature = "g-ruby")]
        "ruby" => tree_sitter_ruby::LANGUAGE.into(),
        #[cfg(feature = "g-scala")]
        "scala" => tree_sitter_scala::LANGUAGE.into(),
        #[cfg(feature = "g-scss")]
        "scss" => tree_sitter_scss::language(),
        #[cfg(feature = "g-solidity")]
        "solidity" => tree_sitter_solidity::LANGUAGE.into(),
        #[cfg(feature = "g-sequel")]
        "sql" => tree_sitter_sequel::LANGUAGE.into(),
        #[cfg(feature = "g-swift")]
        "swift" => tree_sitter_swift::LANGUAGE.into(),
        #[cfg(feature = "g-toml-ng")]
        "toml" => tree_sitter_toml_ng::LANGUAGE.into(),
        #[cfg(feature = "g-verilog")]
        "verilog" => tree_sitter_verilog::LANGUAGE.into(),
        #[cfg(feature = "g-xml")]
        "xml" => tree_sitter_xml::LANGUAGE_XML.into(),
        #[cfg(feature = "g-yaml")]
        "yaml" => tree_sitter_yaml::LANGUAGE.into(),
        #[cfg(feature = "g-zig")]
        "zig" => tree_sitter_zig::LANGUAGE.into(),
        _ => return None,
    })
}

fn main() {
    let name = std::env::args().nth(1).unwrap_or_default();
    let mut p = tree_sitter::Parser::new();
    match lang(&name) {
        Some(l) => {
            p.set_language(&l).expect("version");
            let t = p.parse("", None).expect("parse");
            println!("{} ok {}", name, t.root_node().kind());
        }
        None => println!("unknown"),
    }
}
