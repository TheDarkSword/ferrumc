use indexmap::IndexMap;
use quote::quote;
use serde_json::Value;
use std::collections::HashMap;

use craftflow_nbt::DynNBT;

/// The synced-registry payload for each supported version, in the order of
/// `ferrumc_net_codec::version::ProtocolVersion::ALL`. Both the set of registries and their
/// contents change between releases, so a client has to be sent the payload for the version it
/// speaks rather than the newest one.
const REGISTRY_PAYLOADS: [&[u8]; 10] = [
    include_bytes!("../../../../../assets/data/registry_packets/1.21.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.2.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.4.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.5.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.6.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.8.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.9.json"),
    include_bytes!("../../../../../assets/data/registry_packets/1.21.11.json"),
    include_bytes!("../../../../../assets/data/registry_packets/26.1.json"),
    include_bytes!("../../../../../assets/data/registry_packets/26.2.json"),
];

/// What tag each field of a registry entry carries, asked of the game itself by
/// `scripts/extract_registry_tags.py`. Only the versions whose jars carry their own names can be
/// asked; the older ones use the oldest table there is, since not one of the 754 field paths the
/// two share disagrees between them.
const REGISTRY_TAGS: [&[u8]; 2] = [
    include_bytes!("../../../../../assets/data/registry_tags/26.1.json"),
    include_bytes!("../../../../../assets/data/registry_tags/26.2.json"),
];

/// Which table each version uses, in the order of `ProtocolVersion::ALL`.
const TABLE_FOR_VERSION: [usize; 10] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

pub(crate) fn build_mapping(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let versions = REGISTRY_PAYLOADS
        .iter()
        .zip(TABLE_FOR_VERSION)
        .map(|(payload, table)| build_one(payload, load_tags(REGISTRY_TAGS[table])));
    quote! {
        [#(#versions),*]
    }
    .into()
}

/// Reads a tag table into `registry -> field path -> tag id`.
fn load_tags(table: &[u8]) -> HashMap<String, HashMap<String, u8>> {
    #[derive(serde::Deserialize)]
    struct Table {
        registries: HashMap<String, HashMap<String, Vec<u8>>>,
    }
    let table: Table = serde_json::from_slice(table).expect("the tag table should be valid JSON");
    table
        .registries
        .into_iter()
        .map(|(registry, fields)| {
            let fields = fields
                .into_iter()
                .filter_map(|(path, tags)| {
                    // A path may carry more than one tag where the codec accepts either a number
                    // or a compound written in its place. Only the numeric one is of any use here:
                    // a compound converts correctly on its own.
                    let numeric = tags.iter().copied().find(|tag| (1..=6).contains(tag))?;
                    Some((path, numeric))
                })
                .collect();
            (registry, fields)
        })
        .collect()
}

fn build_one(
    json_file: &[u8],
    tags: HashMap<String, HashMap<String, u8>>,
) -> proc_macro2::TokenStream {
    let val: IndexMap<String, IndexMap<String, Value>> = serde_json::from_slice(json_file).unwrap();

    let mut registry_entries = vec![];

    for (reg_entry, value_set) in val {
        let mut packets = vec![];
        for (value_name, value) in &value_set {
            let mut nbt_data_buf = Vec::new();
            // The payload is built from json, which has one number type where NBT has six. A
            // lenient client coerces them; a strict one reads the payload into typed structs and
            // refuses a field whose tag is not what its schema says. There is no rule to guess by —
            // most numeric fields in these registries are floats, some are ints, and the same name
            // means different things at different depths — so the tag of every field was asked of
            // the game and is looked up here by where the field sits.
            let empty = HashMap::new();
            let fields = tags.get(&reg_entry).unwrap_or(&empty);
            let nbt = json_to_nbt(value, fields, String::new());
            craftflow_nbt::to_writer(&mut nbt_data_buf, &nbt).unwrap();
            let kv = (value_name.clone(), nbt_data_buf);
            packets.push(kv);
        }
        registry_entries.push((reg_entry, packets));
    }
    let pairs = registry_entries
        .iter()
        .map(|(key, packets)| {
            // Emitted as one byte-string literal rather than a comma-separated list of integers.
            // Ten versions of registry data is a few megabytes, and a token per byte is minutes of
            // compile time for this crate alone.
            let raw_packets_data = proc_macro2::Literal::byte_string(&bitcode::encode(packets));
            quote! {
                (#key.to_string(), #raw_packets_data.to_vec())
            }
        })
        .collect::<Vec<_>>();

    quote! {
        indexmap::IndexMap::from([
            #(#pairs),*
        ])
    }
}

/// Converts a registry entry's JSON into NBT, giving every field the tag the game gives it.
///
/// `path` is where this value sits in the entry: `/effects/minecraft:damage[]/effect/value/base`.
/// A field the table says nothing about keeps the sensible default — integers become `Int` rather
/// than `Long`, reals `Double`, booleans `Byte` — which is what the older versions fall back on
/// for anything their own release added.
fn json_to_nbt(value: &Value, fields: &HashMap<String, u8>, path: String) -> DynNBT {
    match value {
        Value::Bool(b) => DynNBT::Byte(i8::from(*b)),
        Value::Number(n) => match fields.get(&path).copied() {
            Some(1) => DynNBT::Byte(num_i64(n) as i8),
            Some(2) => DynNBT::Short(num_i64(n) as i16),
            Some(3) => DynNBT::Int(num_i64(n) as i32),
            Some(4) => DynNBT::Long(num_i64(n)),
            Some(5) => DynNBT::Float(num_f64(n) as f32),
            Some(6) => DynNBT::Double(num_f64(n)),
            _ => {
                if let Some(i) = n.as_i64() {
                    // Widen to a long only where the number genuinely does not fit.
                    match i32::try_from(i) {
                        Ok(v) => DynNBT::Int(v),
                        Err(_) => DynNBT::Long(i),
                    }
                } else {
                    DynNBT::Double(num_f64(n))
                }
            }
        },
        Value::String(s) => DynNBT::String(s.clone()),
        Value::Array(items) => {
            let below = format!("{path}[]");
            DynNBT::List(
                items
                    .iter()
                    .map(|item| json_to_nbt(item, fields, below.clone()))
                    .collect(),
            )
        }
        Value::Object(obj) => {
            let mut map = HashMap::with_capacity(obj.len());
            for (key, value) in obj {
                map.insert(
                    key.clone(),
                    json_to_nbt(value, fields, format!("{path}/{key}")),
                );
            }
            DynNBT::Compound(map)
        }
        // Registries contain no JSON nulls; encode defensively as a zero byte rather than panicking.
        Value::Null => DynNBT::Byte(0),
    }
}

fn num_f64(n: &serde_json::Number) -> f64 {
    n.as_f64()
        .expect("registry numeric value is representable as f64")
}

fn num_i64(n: &serde_json::Number) -> i64 {
    n.as_i64()
        .expect("registry numeric value forced to an integer tag must be an integer")
}
