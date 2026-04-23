#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
use mf2_i18n_core::{Args, DateTimeValue, Value};
use mf2_i18n_runtime::{BasicFormatBackend, Manifest, Runtime, RuntimeError, RuntimeParts};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CONTRACT: &str = r#"
export interface Mf2RuntimeParts {
  manifest: unknown;
  idMap: unknown;
  packs: Record<string, Uint8Array | ArrayBuffer>;
}

export type Mf2Arg =
  | string
  | number
  | boolean
  | { type: "datetime"; unixMilliseconds: number }
  | { type: "datetime"; unixSeconds: number }
  | { type: "currency"; value: number; code: string }
  | { type: "unit"; value: number; unitId: number };
"#;

#[wasm_bindgen(js_name = Mf2Runtime)]
pub struct Mf2Runtime {
    runtime: Runtime,
}

#[wasm_bindgen(js_class = Mf2Runtime)]
impl Mf2Runtime {
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(input: JsValue) -> Result<Mf2Runtime, JsValue> {
        let raw = raw_parts_from_js(&input).map_err(binding_error)?;
        Self::from_raw_parts(raw).map_err(binding_error)
    }

    #[wasm_bindgen(js_name = defaultLocale)]
    pub fn default_locale(&self) -> String {
        self.runtime.default_locale().to_owned()
    }

    #[wasm_bindgen(js_name = supportedLocales)]
    pub fn supported_locales(&self) -> Array {
        let values = Array::new();
        for locale in self.runtime.supported_locales() {
            values.push(&JsValue::from_str(locale.normalized()));
        }
        values
    }

    pub fn format(&self, locale: &str, key: &str, args: JsValue) -> Result<String, JsValue> {
        let args = args_from_js(&args).map_err(binding_error)?;
        self.runtime
            .format_with_backend(locale, key, &args, &BasicFormatBackend)
            .map_err(WasmBindingError::from)
            .map_err(binding_error)
    }
}

impl Mf2Runtime {
    fn from_raw_parts(parts: RawRuntimeParts) -> Result<Self, WasmBindingError> {
        let manifest: Manifest = serde_json::from_slice(&parts.manifest_json)
            .map_err(|source| WasmBindingError::Json("manifest", source.to_string()))?;
        let runtime =
            Runtime::from_parts(RuntimeParts::new(manifest, parts.id_map_json, parts.packs))
                .map_err(WasmBindingError::from)?;
        Ok(Self { runtime })
    }

    #[cfg(test)]
    fn format_json_args(
        &self,
        locale: &str,
        key: &str,
        args: Option<serde_json::Value>,
    ) -> Result<String, WasmBindingError> {
        let args = args_from_json(args)?;
        self.runtime
            .format_with_backend(locale, key, &args, &BasicFormatBackend)
            .map_err(WasmBindingError::from)
    }
}

#[derive(Debug)]
struct RawRuntimeParts {
    manifest_json: Vec<u8>,
    id_map_json: Vec<u8>,
    packs: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug)]
enum WasmBindingError {
    MissingField(&'static str),
    InvalidField(&'static str),
    Json(&'static str, String),
    Runtime(String),
    InvalidArgs(String),
    UnsupportedArgShape(String),
    InvalidCurrencyCode(String),
}

impl fmt::Display for WasmBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(formatter, "missing field `{field}`"),
            Self::InvalidField(field) => write!(formatter, "invalid field `{field}`"),
            Self::Json(field, source) => write!(formatter, "invalid JSON in `{field}`: {source}"),
            Self::Runtime(source) => write!(formatter, "runtime error: {source}"),
            Self::InvalidArgs(message) => write!(formatter, "invalid args: {message}"),
            Self::UnsupportedArgShape(key) => {
                write!(formatter, "unsupported argument shape for `{key}`")
            }
            Self::InvalidCurrencyCode(code) => {
                write!(formatter, "invalid currency code `{code}`")
            }
        }
    }
}

impl From<RuntimeError> for WasmBindingError {
    fn from(source: RuntimeError) -> Self {
        Self::Runtime(source.to_string())
    }
}

fn binding_error(error: WasmBindingError) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}

fn raw_parts_from_js(input: &JsValue) -> Result<RawRuntimeParts, WasmBindingError> {
    if !input.is_object() || input.is_null() {
        return Err(WasmBindingError::InvalidField("input"));
    }
    let manifest_json = json_bytes_from_js("manifest", &required_property(input, "manifest")?)?;
    let id_map_json = id_map_bytes_from_js(&required_property(input, "idMap")?)?;
    let packs = packs_from_js(&required_property(input, "packs")?)?;
    Ok(RawRuntimeParts {
        manifest_json,
        id_map_json,
        packs,
    })
}

fn required_property(input: &JsValue, name: &'static str) -> Result<JsValue, WasmBindingError> {
    let value = Reflect::get(input, &JsValue::from_str(name))
        .map_err(|_| WasmBindingError::InvalidField(name))?;
    if value.is_undefined() || value.is_null() {
        return Err(WasmBindingError::MissingField(name));
    }
    Ok(value)
}

fn json_bytes_from_js(field: &'static str, value: &JsValue) -> Result<Vec<u8>, WasmBindingError> {
    if let Some(text) = value.as_string() {
        return Ok(text.into_bytes());
    }
    let json = js_sys::JSON::stringify(value)
        .map_err(|_| WasmBindingError::InvalidField(field))?
        .as_string()
        .ok_or(WasmBindingError::InvalidField(field))?;
    Ok(json.into_bytes())
}

fn id_map_bytes_from_js(value: &JsValue) -> Result<Vec<u8>, WasmBindingError> {
    if is_binary(value) {
        return binary_bytes_from_js("idMap", value);
    }
    json_bytes_from_js("idMap", value)
}

fn packs_from_js(value: &JsValue) -> Result<BTreeMap<String, Vec<u8>>, WasmBindingError> {
    if !value.is_object() || value.is_null() || Array::is_array(value) {
        return Err(WasmBindingError::InvalidField("packs"));
    }
    let mut packs = BTreeMap::new();
    let object = Object::from(value.clone());
    let entries = Object::entries(&object);
    for index in 0..entries.length() {
        let pair = Array::from(&entries.get(index));
        let locale = pair
            .get(0)
            .as_string()
            .ok_or(WasmBindingError::InvalidField("packs"))?;
        let bytes = binary_bytes_from_js("packs", &pair.get(1))?;
        packs.insert(locale, bytes);
    }
    Ok(packs)
}

fn is_binary(value: &JsValue) -> bool {
    value.is_instance_of::<Uint8Array>() || value.is_instance_of::<ArrayBuffer>()
}

fn binary_bytes_from_js(field: &'static str, value: &JsValue) -> Result<Vec<u8>, WasmBindingError> {
    if value.is_instance_of::<Uint8Array>() {
        return Ok(Uint8Array::new(value).to_vec());
    }
    if value.is_instance_of::<ArrayBuffer>() {
        return Ok(Uint8Array::new(value).to_vec());
    }
    Err(WasmBindingError::InvalidField(field))
}

fn args_from_js(value: &JsValue) -> Result<Args, WasmBindingError> {
    if value.is_undefined() || value.is_null() {
        return Ok(Args::new());
    }
    if !value.is_object() || Array::is_array(value) {
        return Err(WasmBindingError::InvalidArgs(
            "args must be an object record".to_owned(),
        ));
    }
    let mut args = Args::new();
    let object = Object::from(value.clone());
    let entries = Object::entries(&object);
    for index in 0..entries.length() {
        let pair = Array::from(&entries.get(index));
        let key = pair.get(0).as_string().ok_or_else(|| {
            WasmBindingError::InvalidArgs("argument key is not a string".to_owned())
        })?;
        args.insert(key.clone(), arg_value_from_js(&key, &pair.get(1))?);
    }
    Ok(args)
}

fn arg_value_from_js(key: &str, value: &JsValue) -> Result<Value, WasmBindingError> {
    if let Some(text) = value.as_string() {
        return Ok(Value::Str(text));
    }
    if let Some(number) = value.as_f64() {
        return Ok(Value::Num(number));
    }
    if let Some(boolean) = value.as_bool() {
        return Ok(Value::Bool(boolean));
    }
    if !value.is_object() || value.is_null() || Array::is_array(value) {
        return Err(WasmBindingError::UnsupportedArgShape(key.to_owned()));
    }

    let kind = Reflect::get(value, &JsValue::from_str("type"))
        .map_err(|_| WasmBindingError::UnsupportedArgShape(key.to_owned()))?
        .as_string()
        .ok_or_else(|| WasmBindingError::UnsupportedArgShape(key.to_owned()))?;
    match kind.as_str() {
        "datetime" => {
            let seconds = optional_number_property(value, "unixSeconds")?;
            let milliseconds = optional_number_property(value, "unixMilliseconds")?;
            datetime_arg(key, seconds, milliseconds)
        }
        "currency" => currency_arg(
            key,
            required_number_property(value, "value")?,
            &required_string_property(value, "code")?,
        ),
        "unit" => unit_arg(
            key,
            required_number_property(value, "value")?,
            required_number_property(value, "unitId")?,
        ),
        _ => Err(WasmBindingError::UnsupportedArgShape(key.to_owned())),
    }
}

fn optional_number_property(
    value: &JsValue,
    property: &'static str,
) -> Result<Option<f64>, WasmBindingError> {
    let field = Reflect::get(value, &JsValue::from_str(property))
        .map_err(|_| WasmBindingError::InvalidField(property))?;
    if field.is_undefined() || field.is_null() {
        return Ok(None);
    }
    field
        .as_f64()
        .map(Some)
        .ok_or(WasmBindingError::InvalidField(property))
}

fn required_number_property(
    value: &JsValue,
    property: &'static str,
) -> Result<f64, WasmBindingError> {
    optional_number_property(value, property)?.ok_or(WasmBindingError::MissingField(property))
}

fn required_string_property(
    value: &JsValue,
    property: &'static str,
) -> Result<String, WasmBindingError> {
    let field = Reflect::get(value, &JsValue::from_str(property))
        .map_err(|_| WasmBindingError::InvalidField(property))?;
    if field.is_undefined() || field.is_null() {
        return Err(WasmBindingError::MissingField(property));
    }
    field
        .as_string()
        .ok_or(WasmBindingError::InvalidField(property))
}

#[cfg(test)]
fn args_from_json(value: Option<serde_json::Value>) -> Result<Args, WasmBindingError> {
    let Some(value) = value else {
        return Ok(Args::new());
    };
    let serde_json::Value::Object(object) = value else {
        return Err(WasmBindingError::InvalidArgs(
            "args must be an object record".to_owned(),
        ));
    };

    let mut args = Args::new();
    for (key, value) in object {
        args.insert(key.clone(), arg_value_from_json(&key, value)?);
    }
    Ok(args)
}

#[cfg(test)]
fn arg_value_from_json(key: &str, value: serde_json::Value) -> Result<Value, WasmBindingError> {
    match value {
        serde_json::Value::String(text) => Ok(Value::Str(text)),
        serde_json::Value::Number(number) => number
            .as_f64()
            .map(Value::Num)
            .ok_or_else(|| WasmBindingError::UnsupportedArgShape(key.to_owned())),
        serde_json::Value::Bool(boolean) => Ok(Value::Bool(boolean)),
        serde_json::Value::Object(mut object) => {
            let kind = object
                .remove("type")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| WasmBindingError::UnsupportedArgShape(key.to_owned()))?;
            match kind.as_str() {
                "datetime" => {
                    let seconds = optional_json_number(&object, "unixSeconds")?;
                    let milliseconds = optional_json_number(&object, "unixMilliseconds")?;
                    datetime_arg(key, seconds, milliseconds)
                }
                "currency" => currency_arg(
                    key,
                    required_json_number(&object, "value")?,
                    required_json_string(&object, "code")?,
                ),
                "unit" => unit_arg(
                    key,
                    required_json_number(&object, "value")?,
                    required_json_number(&object, "unitId")?,
                ),
                _ => Err(WasmBindingError::UnsupportedArgShape(key.to_owned())),
            }
        }
        _ => Err(WasmBindingError::UnsupportedArgShape(key.to_owned())),
    }
}

#[cfg(test)]
fn optional_json_number(
    object: &serde_json::Map<String, serde_json::Value>,
    property: &'static str,
) -> Result<Option<f64>, WasmBindingError> {
    let Some(value) = object.get(property) else {
        return Ok(None);
    };
    value
        .as_f64()
        .map(Some)
        .ok_or(WasmBindingError::InvalidField(property))
}

#[cfg(test)]
fn required_json_number(
    object: &serde_json::Map<String, serde_json::Value>,
    property: &'static str,
) -> Result<f64, WasmBindingError> {
    optional_json_number(object, property)?.ok_or(WasmBindingError::MissingField(property))
}

#[cfg(test)]
fn required_json_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    property: &'static str,
) -> Result<&'a str, WasmBindingError> {
    object
        .get(property)
        .and_then(|value| value.as_str())
        .ok_or(WasmBindingError::MissingField(property))
}

fn datetime_arg(
    key: &str,
    unix_seconds: Option<f64>,
    unix_milliseconds: Option<f64>,
) -> Result<Value, WasmBindingError> {
    match (unix_seconds, unix_milliseconds) {
        (Some(_), Some(_)) | (None, None) => {
            Err(WasmBindingError::UnsupportedArgShape(key.to_owned()))
        }
        (Some(value), None) => Ok(Value::DateTime(DateTimeValue::unix_seconds(number_to_i64(
            "unixSeconds",
            value,
        )?))),
        (None, Some(value)) => Ok(Value::DateTime(DateTimeValue::unix_milliseconds(
            number_to_i64("unixMilliseconds", value)?,
        ))),
    }
}

fn currency_arg(key: &str, value: f64, code: &str) -> Result<Value, WasmBindingError> {
    if !value.is_finite() {
        return Err(WasmBindingError::UnsupportedArgShape(key.to_owned()));
    }
    let normalized = code.to_ascii_uppercase();
    let bytes = normalized.as_bytes();
    if bytes.len() != 3 || !bytes.iter().all(u8::is_ascii_alphabetic) {
        return Err(WasmBindingError::InvalidCurrencyCode(code.to_owned()));
    }
    Ok(Value::Currency {
        value,
        code: [bytes[0], bytes[1], bytes[2]],
    })
}

fn unit_arg(key: &str, value: f64, unit_id: f64) -> Result<Value, WasmBindingError> {
    if !value.is_finite() {
        return Err(WasmBindingError::UnsupportedArgShape(key.to_owned()));
    }
    Ok(Value::Unit {
        value,
        unit_id: number_to_u32("unitId", unit_id)?,
    })
}

fn number_to_i64(field: &'static str, value: f64) -> Result<i64, WasmBindingError> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < i64::MIN as f64
        || value > i64::MAX as f64
    {
        return Err(WasmBindingError::InvalidField(field));
    }
    Ok(value as i64)
}

fn number_to_u32(field: &'static str, value: f64) -> Result<u32, WasmBindingError> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < u32::MIN as f64
        || value > u32::MAX as f64
    {
        return Err(WasmBindingError::InvalidField(field));
    }
    Ok(value as u32)
}

#[cfg(test)]
mod tests {
    use super::{Mf2Runtime, RawRuntimeParts, WasmBindingError, args_from_json};
    use mf2_i18n_build::{ProjectRuntimeBuildOptions, build_project_runtime_artifacts};
    use mf2_i18n_core::{DateTimeValue, Value};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_wasm_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_project(root: &Path) -> PathBuf {
        let locale_dir = root.join("locales").join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(
            locale_dir.join("common.json"),
            r#"{"home":{"title":"Hi { $name }"}}"#,
        )
        .expect("catalog");
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");
        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");
        config_path
    }

    fn runtime_fixture() -> (PathBuf, Mf2Runtime) {
        let root = temp_dir("runtime");
        let config_path = write_project(&root);
        let output = build_project_runtime_artifacts(&ProjectRuntimeBuildOptions::new(
            &config_path,
            root.join("out"),
            "r1",
            "2026-02-01T00:00:00Z",
        ))
        .expect("build");

        let manifest_json = fs::read(output.manifest_path()).expect("manifest");
        let id_map_json = fs::read(output.id_map_path()).expect("id map");
        let mut packs = BTreeMap::new();
        packs.insert(
            "en".to_owned(),
            fs::read(output.packs_dir().join("en.mf2pack")).expect("pack"),
        );
        let runtime = Mf2Runtime::from_raw_parts(RawRuntimeParts {
            manifest_json,
            id_map_json,
            packs,
        })
        .expect("runtime");
        (root, runtime)
    }

    #[test]
    fn raw_parts_constructor_formats_message() {
        let (root, runtime) = runtime_fixture();
        assert_eq!(runtime.default_locale(), "en");
        let output = runtime
            .format_json_args(
                "en",
                "common.home.title",
                Some(serde_json::json!({ "name": "Nova" })),
            )
            .expect("format");
        assert_eq!(output, "Hi Nova");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn json_argument_conversion_supports_approved_shapes() {
        let args = args_from_json(Some(serde_json::json!({
            "name": "Nova",
            "count": 3,
            "ready": true,
            "created": { "type": "datetime", "unixSeconds": 994550400 },
            "updated": { "type": "datetime", "unixMilliseconds": 994550400000_i64 },
            "price": { "type": "currency", "value": 12.5, "code": "usd" },
            "distance": { "type": "unit", "value": 7.25, "unitId": 42 }
        })))
        .expect("args");

        assert!(matches!(args.get("name"), Some(Value::Str(value)) if value == "Nova"));
        assert!(matches!(args.get("count"), Some(Value::Num(value)) if *value == 3.0));
        assert!(matches!(args.get("ready"), Some(Value::Bool(true))));
        assert!(matches!(
            args.get("created"),
            Some(Value::DateTime(DateTimeValue::UnixSeconds(994550400)))
        ));
        assert!(matches!(
            args.get("updated"),
            Some(Value::DateTime(DateTimeValue::UnixMilliseconds(
                994550400000
            )))
        ));
        assert!(
            matches!(args.get("price"), Some(Value::Currency { value, code }) if *value == 12.5 && code == b"USD")
        );
        assert!(
            matches!(args.get("distance"), Some(Value::Unit { value, unit_id }) if *value == 7.25 && *unit_id == 42)
        );
    }

    #[test]
    fn json_argument_conversion_rejects_unsupported_shapes() {
        let err = match args_from_json(Some(serde_json::json!({
            "bad": { "value": 1 }
        }))) {
            Ok(_) => panic!("unsupported shape should fail"),
            Err(error) => error,
        };
        assert!(matches!(err, WasmBindingError::UnsupportedArgShape(key) if key == "bad"));
    }

    #[test]
    fn json_argument_conversion_rejects_invalid_currency_code() {
        let err = match args_from_json(Some(serde_json::json!({
            "price": { "type": "currency", "value": 1, "code": "US" }
        }))) {
            Ok(_) => panic!("invalid currency should fail"),
            Err(error) => error,
        };
        assert!(matches!(err, WasmBindingError::InvalidCurrencyCode(code) if code == "US"));
    }
}
