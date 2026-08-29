//! Generates `{perp,lp}_events.rs` and `{perp,lp}_accounts.rs` (structs, enums, discriminators)
//! from each program's Anchor IDL, so the hand-written `src/parser/**/events.rs` and
//! `accounts.rs` files never have to be edited when a program's events/accounts change.
//!
//! Each IDL path defaults to the matching sibling `programs/*` build output, but can be
//! overridden with an env var — e.g. a future CI job that rebuilds/fetches IDLs elsewhere and
//! points this build at those files instead.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn main() {
    process_idl(
        "PERP_IDL_PATH",
        "programs/perp/target/idl/perp.json",
        "perp_events.rs",
        "perp_accounts.rs",
        "PerpEvent",
    );
    process_idl(
        "LP_POOL_IDL_PATH",
        "programs/liquidity-pool/target/idl/liquidity_pool.json",
        "lp_events.rs",
        "lp_accounts.rs",
        "LpEvent",
    );
}

fn process_idl(
    env_var: &str,
    default_relative_path: &str,
    events_file: &str,
    accounts_file: &str,
    event_enum_name: &str,
) {
    println!("cargo:rerun-if-env-changed={env_var}");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let idl_path =
        env::var(env_var).unwrap_or_else(|_| format!("{manifest_dir}/../{default_relative_path}"));

    println!("cargo:rerun-if-changed={idl_path}");

    let idl_raw = fs::read_to_string(&idl_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read IDL at `{idl_path}`: {e}\n\
             Run `anchor build` for the matching program first, or set {env_var}."
        )
    });

    let idl: Value = serde_json::from_str(&idl_raw).unwrap_or_else(|e| panic!("IDL at `{idl_path}` is not valid JSON: {e}"));
    let program_name = idl["metadata"]["name"].as_str().unwrap_or("unknown");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    fs::write(out_dir.join(events_file), generate_events(&idl, program_name, event_enum_name))
        .unwrap_or_else(|e| panic!("Failed to write {events_file}: {e}"));
    fs::write(out_dir.join(accounts_file), generate_accounts(&idl, program_name))
        .unwrap_or_else(|e| panic!("Failed to write {accounts_file}: {e}"));
}

fn generate_events(idl: &Value, program_name: &str, enum_name: &str) -> String {
    let types = idl["types"].as_array().cloned().unwrap_or_default();
    let events = idl["events"].as_array().cloned().unwrap_or_default();
    assert!(!events.is_empty(), "{program_name} IDL has no events");

    let mut out = format!("// @generated from the {program_name} program IDL by build.rs — do not edit by hand.\n\n");
    let mut emitted = BTreeSet::new();
    let mut variants = String::new();

    for event in &events {
        let name = event["name"].as_str().expect("event missing `name`");
        emit_named_with_discriminator(&mut out, &mut emitted, &types, event, name, "AnchorEvent");
        variants.push_str(&format!("    {name}({name}),\n"));
    }

    out.push_str(&format!(
        "/// Every event the `{program_name}` program emits, decoded from a `Program data:` log line.\n"
    ));
    out.push_str(&format!("#[derive(Debug, Clone, PartialEq)]\npub enum {enum_name} {{\n"));
    out.push_str(&variants);
    out.push_str("}\n");

    out
}

/// Unlike events (any log line could be any event, so callers need the event enum to match on),
/// account fetches always know the type they expect up front — no wrapping enum needed.
fn generate_accounts(idl: &Value, program_name: &str) -> String {
    let types = idl["types"].as_array().cloned().unwrap_or_default();
    let accounts = idl["accounts"].as_array().cloned().unwrap_or_default();
    assert!(!accounts.is_empty(), "{program_name} IDL has no accounts");

    let mut out = format!("// @generated from the {program_name} program IDL by build.rs — do not edit by hand.\n\n");
    let mut emitted = BTreeSet::new();

    for account in &accounts {
        let name = account["name"].as_str().expect("account missing `name`");
        emit_named_with_discriminator(&mut out, &mut emitted, &types, account, name, "AnchorAccount");
    }

    out
}

fn emit_named_with_discriminator(
    out: &mut String,
    emitted: &mut BTreeSet<String>,
    types: &[Value],
    entry: &Value,
    name: &str,
    trait_name: &str,
) {
    let discriminator: Vec<String> = entry["discriminator"]
        .as_array()
        .unwrap_or_else(|| panic!("`{name}` missing `discriminator`"))
        .iter()
        .map(|b| b.as_u64().unwrap().to_string())
        .collect();

    let ty = find_type(types, name)
        .unwrap_or_else(|| panic!("`{name}` has no matching entry in IDL `types`"));
    emit_type(out, emitted, types, ty);

    out.push_str(&format!(
        "impl {trait_name} for {name} {{\n    const DISCRIMINATOR: [u8; 8] = [{}];\n}}\n\n",
        discriminator.join(", ")
    ));
}

fn find_type<'a>(types: &'a [Value], name: &str) -> Option<&'a Value> {
    types.iter().find(|t| t["name"] == name)
}

/// Emits a struct or a fieldless enum, recursively emitting whatever named types it depends on
/// first. Structs and enums share the "have we emitted this name already" dedup set since IDL
/// type names are unique across both kinds.
fn emit_type(out: &mut String, emitted: &mut BTreeSet<String>, types: &[Value], ty: &Value) {
    let name = ty["name"].as_str().expect("type missing `name`").to_string();
    if !emitted.insert(name.clone()) {
        return;
    }

    match ty["type"]["kind"].as_str() {
        Some("struct") => emit_struct(out, emitted, types, &name, ty),
        Some("enum") => emit_enum(out, &name, ty),
        other => panic!("type `{name}` has unsupported IDL kind: {other:?}"),
    }
}

fn emit_struct(out: &mut String, emitted: &mut BTreeSet<String>, types: &[Value], name: &str, ty: &Value) {
    let fields = ty["type"]["fields"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("type `{name}` is not a struct with named fields"));

    for field in &fields {
        emit_dependencies(out, emitted, types, &field["type"]);
    }

    out.push_str("#[derive(Debug, Clone, PartialEq, BorshDeserialize)]\npub struct ");
    out.push_str(name);
    out.push_str(" {\n");
    for field in &fields {
        let field_name = field["name"].as_str().expect("field missing `name`");
        out.push_str(&format!("    pub {field_name}: {},\n", rust_type(&field["type"])));
    }
    out.push_str("}\n\n");
}

fn emit_enum(out: &mut String, name: &str, ty: &Value) {
    let variants = ty["type"]["variants"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("type `{name}` is not an enum with variants"));

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshDeserialize)]\npub enum ");
    out.push_str(name);
    out.push_str(" {\n");
    for variant in &variants {
        let variant_name = variant["name"].as_str().expect("variant missing `name`");
        assert!(
            variant.get("fields").is_none(),
            "enum `{name}` variant `{variant_name}` has fields — only fieldless (unit) variants are supported"
        );
        out.push_str(&format!("    {variant_name},\n"));
    }
    out.push_str("}\n\n");
}

/// Recursively emits definitions for any `defined` (named) type a field depends on, before the
/// field's own containing type is emitted.
fn emit_dependencies(out: &mut String, emitted: &mut BTreeSet<String>, types: &[Value], field_type: &Value) {
    if let Some(defined) = field_type.get("defined") {
        let name = defined["name"].as_str().expect("`defined` missing `name`");
        let ty = find_type(types, name)
            .unwrap_or_else(|| panic!("no IDL type definition found for `{name}`"));
        emit_type(out, emitted, types, ty);
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
            "string" => "String".to_string(),
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
