//! Generates `perp_events.rs` (structs + discriminators + the `PerpEvent` enum) from the
//! perp program's Anchor IDL, so `src/parser/events.rs` never has to be hand-edited when
//! the program's events change.
//!
//! The IDL path defaults to the sibling `programs/perp` build output, but can be overridden
//! with `PERP_IDL_PATH` — e.g. a future CI job that rebuilds/fetches the IDL elsewhere and
//! points this build at that file instead.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn main() {
    println!("cargo:rerun-if-env-changed=PERP_IDL_PATH");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let idl_path = env::var("PERP_IDL_PATH")
        .unwrap_or_else(|_| format!("{manifest_dir}/../programs/perp/target/idl/perp.json"));

    println!("cargo:rerun-if-changed={idl_path}");

    let idl_raw = fs::read_to_string(&idl_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read perp IDL at `{idl_path}`: {e}\n\
             Run `anchor build` in programs/perp first, or point PERP_IDL_PATH at a built IDL."
        )
    });

    let idl: Value = serde_json::from_str(&idl_raw).expect("perp IDL is not valid JSON");
    let generated = generate_events(&idl);

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("perp_events.rs");
    fs::write(out_path, generated).expect("Failed to write generated perp events");
}

fn generate_events(idl: &Value) -> String {
    let types = idl["types"].as_array().cloned().unwrap_or_default();
    let events = idl["events"].as_array().cloned().unwrap_or_default();
    assert!(!events.is_empty(), "perp IDL has no events");

    let mut out = String::from("// @generated from the perp program IDL by build.rs — do not edit by hand.\n\n");
    let mut emitted = BTreeSet::new();
    let mut variants = String::new();

    for event in &events {
        let name = event["name"].as_str().expect("event missing `name`");
        let discriminator: Vec<String> = event["discriminator"]
            .as_array()
            .unwrap_or_else(|| panic!("event `{name}` missing `discriminator`"))
            .iter()
            .map(|b| b.as_u64().unwrap().to_string())
            .collect();

        let ty = find_type(&types, name)
            .unwrap_or_else(|| panic!("event `{name}` has no matching entry in IDL `types`"));
        emit_struct(&mut out, &mut emitted, &types, ty);

        out.push_str(&format!(
            "impl AnchorEvent for {name} {{\n    const DISCRIMINATOR: [u8; 8] = [{}];\n}}\n\n",
            discriminator.join(", ")
        ));

        variants.push_str(&format!("    {name}({name}),\n"));
    }

    out.push_str("/// Every event the `perp` program emits, decoded from a `Program data:` log line.\n");
    out.push_str("#[derive(Debug, Clone, PartialEq)]\npub enum PerpEvent {\n");
    out.push_str(&variants);
    out.push_str("}\n");

    out
}

fn find_type<'a>(types: &'a [Value], name: &str) -> Option<&'a Value> {
    types.iter().find(|t| t["name"] == name)
}

fn emit_struct(out: &mut String, emitted: &mut BTreeSet<String>, types: &[Value], ty: &Value) {
    let name = ty["name"].as_str().expect("type missing `name`").to_string();
    if !emitted.insert(name.clone()) {
        return;
    }

    let fields = ty["type"]["fields"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("type `{name}` is not a struct with named fields"));

    for field in &fields {
        emit_dependencies(out, emitted, types, &field["type"]);
    }

    out.push_str("#[derive(Debug, Clone, PartialEq, BorshDeserialize)]\npub struct ");
    out.push_str(&name);
    out.push_str(" {\n");
    for field in &fields {
        let field_name = field["name"].as_str().expect("field missing `name`");
        out.push_str(&format!("    pub {field_name}: {},\n", rust_type(&field["type"])));
    }
    out.push_str("}\n\n");
}

/// Recursively emits struct definitions for any `defined` (named) type a field depends on,
/// before the field's own containing struct is emitted.
fn emit_dependencies(out: &mut String, emitted: &mut BTreeSet<String>, types: &[Value], field_type: &Value) {
    if let Some(defined) = field_type.get("defined") {
        let name = defined["name"].as_str().expect("`defined` missing `name`");
        let ty = find_type(types, name)
            .unwrap_or_else(|| panic!("no IDL type definition found for `{name}`"));
        emit_struct(out, emitted, types, ty);
    } else if let Some(inner) = field_type.get("option") {
        emit_dependencies(out, emitted, types, inner);
    } else if let Some(arr) = field_type.get("array") {
        emit_dependencies(out, emitted, types, &arr[0]);
    }
}

fn rust_type(field_type: &Value) -> String {
    if let Some(primitive) = field_type.as_str() {
        return match primitive {
            "pubkey" => "Pubkey".to_string(),
            "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" | "bool" => {
                primitive.to_string()
            }
            other => panic!("unsupported IDL primitive type: {other}"),
        };
    }

    if let Some(defined) = field_type.get("defined") {
        return defined["name"].as_str().expect("`defined` missing `name`").to_string();
    }

    if let Some(inner) = field_type.get("option") {
        return format!("Option<{}>", rust_type(inner));
    }

    if let Some(arr) = field_type.get("array") {
        let inner = rust_type(&arr[0]);
        let len = arr[1].as_u64().expect("array length must be an integer");
        return format!("[{inner}; {len}]");
    }

    panic!("unsupported IDL field type shape: {field_type}");
}
