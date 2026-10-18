//! Entity types of 10 §4 (and the owning sections), with the envelope of 10 §2.2.
//!
//! **Read-only by construction.** Every field is `pub(crate)`; other crates get `&` accessors only and can obtain
//! values solely by validated parsing ([`crate::object::from_json`]) or by the constructors in this crate that build
//! new values. No type here has a public `&mut self` method, so holding a `&T` (which is all a document hands out)
//! never allows a change. Document mutation is the executor's exclusive capability (plan §3.2, C1.2).
//!
//! **Field order is schema order** (10 JSON-02): envelope (`id`/`colourId`, `type`, `rev`, `name`), the type's
//! properties, then `meta` and `ext`.

use std::borrow::Cow;
use std::marker::PhantomData;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::id::Id;
use crate::types::ObjectType;

/// Declares a model struct with `pub(crate)` fields and a public read accessor per field.
macro_rules! model_struct {
    (
        $(#[$sattr:meta])*
        pub struct $name:ident {
            $(
                $(#[$fattr:meta])*
                $field:ident : $ty:ty
            ),+ $(,)?
        }
    ) => {
        $(#[$sattr])*
        pub struct $name {
            $( $(#[$fattr])* pub(crate) $field: $ty, )+
        }

        impl $name {
            $(
                #[doc = concat!("Read access to `", stringify!($field), "`.")]
                #[must_use]
                pub fn $field(&self) -> &$ty {
                    &self.$field
                }
            )+
        }
    };
}

/// Implements [`ModelObject`] for an envelope-bearing type.
macro_rules! impl_object {
    ($ty:ty, $variant:ident, name) => {
        impl $crate::entities::ModelObject for $ty {
            const TYPE: $crate::types::ObjectType = $crate::types::ObjectType::$variant;
            fn object_id(&self) -> &$crate::id::Id {
                &self.id
            }
            fn object_rev(&self) -> u64 {
                self.rev
            }
            fn object_name(&self) -> Option<&str> {
                Some(&self.name)
            }
        }
    };
    ($ty:ty, $variant:ident, optional_name) => {
        impl $crate::entities::ModelObject for $ty {
            const TYPE: $crate::types::ObjectType = $crate::types::ObjectType::$variant;
            fn object_id(&self) -> &$crate::id::Id {
                &self.id
            }
            fn object_rev(&self) -> u64 {
                self.rev
            }
            fn object_name(&self) -> Option<&str> {
                self.name.as_deref()
            }
        }
    };
    ($ty:ty, $variant:ident) => {
        impl $crate::entities::ModelObject for $ty {
            const TYPE: $crate::types::ObjectType = $crate::types::ObjectType::$variant;
            fn object_id(&self) -> &$crate::id::Id {
                &self.id
            }
            fn object_rev(&self) -> u64 {
                self.rev
            }
            fn object_name(&self) -> Option<&str> {
                None
            }
        }
    };
}

pub mod asset;
pub mod channel;
pub mod drawing;
pub mod effect;
pub mod element;
pub mod library;
pub mod palette;
pub mod pen;
pub mod project;
pub mod scene;
pub mod sound;
pub mod transition;

/// Every addressable object of the model.
pub trait ModelObject: Serialize + DeserializeOwned + JsonSchema + Clone + Send + Sync + 'static {
    /// The object type.
    const TYPE: ObjectType;
    /// The object's ID (`colourId` for swatches).
    fn object_id(&self) -> &Id;
    /// The `documentRev` of the last transaction that changed the object (10 REV-02).
    fn object_rev(&self) -> u64;
    /// The name, for named types.
    fn object_name(&self) -> Option<&str>;
}

/// Free-form metadata (10 §2.2 `meta`): never interpreted by the application.
pub type Meta = Map<String, Value>;

/// The pattern of an `ext` namespace key: reverse-DNS, at least two labels (10 §2.2).
pub const EXT_KEY_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9-]*(\\.[A-Za-z0-9][A-Za-z0-9-]*)+$";

/// Whether `key` is a valid `ext` namespace.
#[must_use]
pub fn is_ext_key(key: &str) -> bool {
    let mut labels = 0;
    for label in key.split('.') {
        let mut chars = label.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphanumeric() => {}
            _ => return false,
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
        labels += 1;
    }
    labels >= 2
}

/// Extension data keyed by a reverse-DNS namespace (10 §2.2 `ext`). Unknown namespaces are preserved verbatim.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ext(Map<String, Value>);

impl Ext {
    /// The namespaces and their data.
    #[must_use]
    pub fn as_map(&self) -> &Map<String, Value> {
        &self.0
    }

    /// Whether there is no extension data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Ext {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Ext {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = Map::<String, Value>::deserialize(d)?;
        if let Some(bad) = map.keys().find(|k| !is_ext_key(k)) {
            return Err(serde::de::Error::custom(format!("`ext` key `{bad}` is not a reverse-DNS namespace")));
        }
        Ok(Ext(map))
    }
}

impl JsonSchema for Ext {
    fn schema_name() -> Cow<'static, str> {
        "Ext".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "propertyNames": { "pattern": EXT_KEY_PATTERN },
            "description": "Extension data keyed by reverse-DNS namespace; unknown namespaces are preserved verbatim."
        })
    }
}

/// `skip_serializing_if` helper for `meta`.
pub(crate) fn meta_is_empty(m: &Meta) -> bool {
    m.is_empty()
}

/// `skip_serializing_if` helper for `ext`.
pub(crate) fn ext_is_empty(e: &Ext) -> bool {
    e.is_empty()
}

/// A stub: `{id, type, name}` standing for an object that is not expanded (10 §2.2). A stub has no `rev`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stub {
    id: Id,
    ty: ObjectType,
    name: Option<String>,
}

impl Stub {
    /// A stub from its parts.
    #[must_use]
    pub fn new(id: Id, ty: ObjectType, name: Option<String>) -> Self {
        Self { id, ty, name }
    }

    /// The ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The object type.
    #[must_use]
    pub fn ty(&self) -> ObjectType {
        self.ty
    }

    /// The name, for named types.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The stub as JSON (`colourId` instead of `id` for swatches).
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert(self.ty.id_field().to_owned(), Value::String(self.id.as_str().to_owned()));
        m.insert("type".to_owned(), Value::String(self.ty.as_str().to_owned()));
        if let Some(n) = &self.name {
            m.insert("name".to_owned(), Value::String(n.clone()));
        }
        Value::Object(m)
    }

    /// Reads a stub of the expected type. Full objects (with `rev`) are rejected: inside a package entry, children
    /// that live in their own entry appear only as stubs (10 §2.2, §6.3).
    ///
    /// # Errors
    /// A message describing the problem.
    pub fn from_json(v: &Value, expected: ObjectType) -> Result<Self, String> {
        let m = v.as_object().ok_or("a stub must be an object")?;
        let id_field = expected.id_field();
        let id = m.get(id_field).and_then(Value::as_str).ok_or(format!("stub needs `{id_field}`"))?;
        let id = Id::parse(id).map_err(|e| e.message().to_owned())?;
        let ty = m.get("type").and_then(Value::as_str).ok_or("stub needs `type`")?;
        if ty != expected.as_str() {
            return Err(format!("expected a `{expected}` stub, found `{ty}`"));
        }
        if m.contains_key("rev") {
            return Err(format!("`{expected}` children are stored in their own entry; only stubs are allowed here"));
        }
        let name = match m.get("name") {
            None => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return Err("stub `name` must be a string".into()),
        };
        if let Some(extra) = m.keys().find(|k| !matches!(k.as_str(), "id" | "colourId" | "type" | "name")) {
            return Err(format!("unexpected stub property `{extra}`"));
        }
        Ok(Self { id, ty: expected, name })
    }
}

/// The JSON Schema of a stub of any type.
#[must_use]
pub fn stub_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "description": "A stub standing for an object that is not expanded (10 §2.2). A stub has no `rev`.",
        "properties": {
            "id": { "$ref": "#/$defs/Id" },
            "colourId": { "$ref": "#/$defs/Id" },
            "type": { "$ref": "#/$defs/ObjectType" },
            "name": { "type": "string" }
        },
        "required": ["type"],
        "oneOf": [ { "required": ["id"] }, { "required": ["colourId"] } ],
        "additionalProperties": false
    })
}

/// An ordered list of children that the package stores in their own entries (10 LAY-01): a project's palettes,
/// drawing banks and scenes, a scene's elements, a bank's drawings. In memory only the IDs are held; the document
/// serialiser expands each to a stub or the full object according to the requested depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildList<T> {
    ids: Vec<Id>,
    _child: PhantomData<fn() -> T>,
}

impl<T: ModelObject> ChildList<T> {
    /// A list from IDs.
    #[must_use]
    pub fn new(ids: Vec<Id>) -> Self {
        Self { ids, _child: PhantomData }
    }

    /// The child IDs in order.
    #[must_use]
    pub fn ids(&self) -> &[Id] {
        &self.ids
    }
}

impl<T> Default for ChildList<T> {
    fn default() -> Self {
        Self { ids: Vec::new(), _child: PhantomData }
    }
}

impl<T: ModelObject> Serialize for ChildList<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let stubs: Vec<Value> = self.ids.iter().map(|id| Stub::new(id.clone(), T::TYPE, None).to_json()).collect();
        stubs.serialize(s)
    }
}

impl<'de, T: ModelObject> Deserialize<'de> for ChildList<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let items = Vec::<Value>::deserialize(d)?;
        let ids = items
            .iter()
            .map(|v| Stub::from_json(v, T::TYPE).map(|s| s.id).map_err(serde::de::Error::custom))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::new(ids))
    }
}

impl<T: ModelObject> JsonSchema for ChildList<T> {
    fn schema_name() -> Cow<'static, str> {
        format!("ChildList_{}", T::schema_name()).into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        // The child's schema accepts the full child or its stub (constant `type`), so another type's stub fails.
        let child = generator.subschema_for::<T>();
        json_schema!({ "type": "array", "items": child, "x-childList": T::TYPE.as_str() })
    }
}

/// A single child stored in its own entry (the project's local library).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRef<T> {
    id: Id,
    _child: PhantomData<fn() -> T>,
}

impl<T: ModelObject> ChildRef<T> {
    /// A reference to a child.
    #[must_use]
    pub fn new(id: Id) -> Self {
        Self { id, _child: PhantomData }
    }

    /// The child ID.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.id
    }
}

impl<T: ModelObject> Serialize for ChildRef<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Stub::new(self.id.clone(), T::TYPE, None).to_json().serialize(s)
    }
}

impl<'de, T: ModelObject> Deserialize<'de> for ChildRef<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        Stub::from_json(&v, T::TYPE).map(|s| Self::new(s.id)).map_err(serde::de::Error::custom)
    }
}

impl<T: ModelObject> JsonSchema for ChildRef<T> {
    fn schema_name() -> Cow<'static, str> {
        format!("ChildRef_{}", T::schema_name()).into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let child = generator.subschema_for::<T>();
        json_schema!({ "allOf": [child], "x-childRef": T::TYPE.as_str() })
    }
}

/// `skip_serializing_if` helper.
pub(crate) fn str_is_empty(s: &str) -> bool {
    s.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_keys_are_reverse_dns() {
        for ok in ["com.example.tool", "app.migrated", "a.b", "x-1.y2"] {
            assert!(is_ext_key(ok), "{ok}");
        }
        for bad in ["", "tool", ".a", "a.", "a..b", "-a.b", "a b.c", "é.com"] {
            assert!(!is_ext_key(bad), "{bad}");
        }
        assert!(serde_json::from_str::<Ext>("{\"tool\": 1}").is_err());
        assert!(serde_json::from_str::<Ext>("{\"com.x\": {\"any\": [1]}}").is_ok());
    }

    #[test]
    fn stubs_reject_full_objects_and_foreign_types() {
        let ok = serde_json::json!({"id": "sc_1", "type": "scene", "name": "A"});
        assert_eq!(Stub::from_json(&ok, ObjectType::Scene).unwrap().name(), Some("A"));
        let full = serde_json::json!({"id": "sc_1", "type": "scene", "rev": 1, "name": "A"});
        assert!(Stub::from_json(&full, ObjectType::Scene).is_err());
        assert!(Stub::from_json(&ok, ObjectType::Palette).is_err());
        let sw = serde_json::json!({"colourId": "col_1", "type": "swatch", "name": "Skin"});
        assert_eq!(Stub::from_json(&sw, ObjectType::Swatch).unwrap().to_json(), sw);
    }
}
