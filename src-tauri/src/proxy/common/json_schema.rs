use serde_json::Value;

/// recursive cleanup JSON Schema to comply with Gemini InterfaceRequire
///
/// 1. [New] Expand $ref 和 $defs: 将ReferenceReplace with actual definition，solve Gemini Not supported $ref question
/// 2. RemoveNot supported的Field: $schema, additionalProperties, format, default, uniqueItems, validation fields
/// 3. HandleUnion type: ["string", "null"] -> "string"
/// 4. [NEW] Handle anyOf Union type: anyOf: [{"type": "string"}, {"type": "null"}] -> "type": "string"
/// 5. 将 type Field的ValueConvertfor lowercase (Gemini v1internal Require)
/// 6. RemovenumberValidation field: multipleOf, exclusiveMinimum, exclusiveMaximum 等
pub fn clean_json_schema(value: &mut Value) {
    // 0. 预Handle：Expand $ref (Schema Flattening)
    // [FIX #952] Recursive collectionAllHierarchy的 $defs/definitions，rather than just from the rootHierarchyextract
    let mut all_defs = serde_json::Map::new();
    collect_all_defs(value, &mut all_defs);

    // Remove根Hierarchy的 $defs/definitions (keep backwardsCompatible)
    if let Value::Object(map) = value {
        map.remove("$defs");
        map.remove("definitions");
    }

    // [FIX #952] alwaysRun flatten_refs，even though defs 为Empty
    // soCancapture andHandleUnable to parse的 $ref (Fallback为 string Type)
    if let Value::Object(map) = value {
        flatten_refs(map, &all_defs);
    }

    // recursive cleanup
    clean_json_schema_recursive(value);
}

/// [NEW #952] Recursive collectionAllHierarchy的 $defs 和 definitions
///
/// MCP Tool的 schema Mayin any nestedHierarchydefinition $defs，not just at the rootHierarchy。
/// 此FunctionDepthTraverse the entire schema，collectAlldefined to a unified map 中。
fn collect_all_defs(value: &Value, defs: &mut serde_json::Map<String, Value>) {
    if let Value::Object(map) = value {
        // collectCurrentHierarchy的 $defs
        if let Some(Value::Object(d)) = map.get("$defs") {
            for (k, v) in d {
                // avoid overwritingAlready existsdefinition（Whichever is defined first takes precedence）
                defs.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        // collectCurrentHierarchy的 definitions (Draft-07 style)
        if let Some(Value::Object(d)) = map.get("definitions") {
            for (k, v) in d {
                defs.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        // Recursive processingAll子Node
        for (key, v) in map {
            // jump over $defs/definitions itself，avoidDuplicateHandle
            if key != "$defs" && key != "definitions" {
                collect_all_defs(v, defs);
            }
        }
    } else if let Value::Array(arr) = value {
        for item in arr {
            collect_all_defs(item, defs);
        }
    }
}

/// recursionExpand $ref
fn flatten_refs(map: &mut serde_json::Map<String, Value>, defs: &serde_json::Map<String, Value>) {
    // Checkand replace $ref
    if let Some(Value::String(ref_path)) = map.remove("$ref") {
        // ParseReference名 (For example #/$defs/MyType -> MyType)
        let ref_name = ref_path.split('/').last().unwrap_or(&ref_path);

        if let Some(def_schema) = defs.get(ref_name) {
            // will be definedContentMerge到Current map
            if let Value::Object(def_map) = def_schema {
                for (k, v) in def_map {
                    // 仅WhenCurrent map None该 key Insert only when (avoid overwriting)
                    // But usually $ref Node不ShouldThere are otherProperty
                    map.entry(k.clone()).or_insert_with(|| v.clone());
                }

                // Recursive processingjustMergeComing inContent中MayPacketContaining $ref
                // Notice：HereMayWill recurse infinitelyIfThere is a cycleReference，但ToolDefinition usuallyYes DAG
                flatten_refs(map, defs);
            }
        } else {
            // [FIX #952] Unable to parse的 $ref: Convertfor loose string Type，avoid API 400 Error
            // This is better than lettingRequestFailedBe good，At leastToolThe call can still be madeLine
            map.insert("type".to_string(), serde_json::json!("string"));
            let hint = format!("(Unresolved $ref: {})", ref_path);
            let desc_val = map
                .entry("description".to_string())
                .or_insert_with(|| Value::String(String::new()));
            if let Value::String(s) = desc_val {
                if !s.contains(&hint) {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    s.push_str(&hint);
                }
            }
        }
    }

    // Traverse subNode
    for (_, v) in map.iter_mut() {
        if let Value::Object(child_map) = v {
            flatten_refs(child_map, defs);
        } else if let Value::Array(arr) = v {
            for item in arr {
                if let Value::Object(item_map) = item {
                    flatten_refs(item_map, defs);
                }
            }
        }
    }
}

fn clean_json_schema_recursive(value: &mut Value) -> bool {
    let mut is_effectively_nullable = false;

    match value {
        Value::Object(map) => {
            // 0. [NEW] Merge allOf
            merge_all_of(map);

            // 1. [CRITICAL] Deep recursive processingchild
            if let Some(Value::Object(props)) = map.get_mut("properties") {
                let mut nullable_keys = std::collections::HashSet::new();
                for (k, v) in props {
                    if clean_json_schema_recursive(v) {
                        nullable_keys.insert(k.clone());
                    }
                }

                if !nullable_keys.is_empty() {
                    if let Some(Value::Array(req_arr)) = map.get_mut("required") {
                        req_arr.retain(|r| {
                            r.as_str()
                                .map(|s| !nullable_keys.contains(s))
                                .unwrap_or(true)
                        });
                        if req_arr.is_empty() {
                            map.remove("required");
                        }
                    }
                }
            } else if let Some(items) = map.get_mut("items") {
                clean_json_schema_recursive(items);
            } else {
                for v in map.values_mut() {
                    clean_json_schema_recursive(v);
                }
            }

            // 1.5. [FIX] recursive cleanup anyOf/oneOf ArrayinEachbranch
            // Must在MergelogicBeforeExecute，make sureMergebranchesAlreadybe cleansed
            if let Some(Value::Array(any_of)) = map.get_mut("anyOf") {
                for branch in any_of.iter_mut() {
                    clean_json_schema_recursive(branch);
                }
            }
            if let Some(Value::Array(one_of)) = map.get_mut("oneOf") {
                for branch in one_of.iter_mut() {
                    clean_json_schema_recursive(branch);
                }
            }

            // 2. [FIX #815] Handle anyOf/oneOf Union type: MergePropertyrather than directlyDelete
            let mut union_to_merge = None;
            if map.get("type").is_none()
                || map.get("type").and_then(|t| t.as_str()) == Some("object")
            {
                if let Some(Value::Array(any_of)) = map.get("anyOf") {
                    union_to_merge = Some(any_of.clone());
                } else if let Some(Value::Array(one_of)) = map.get("oneOf") {
                    union_to_merge = Some(one_of.clone());
                }
            }

            if let Some(union_array) = union_to_merge {
                if let Some(best_branch) = extract_best_schema_from_union(&union_array) {
                    if let Value::Object(branch_obj) = best_branch {
                        for (k, v) in branch_obj {
                            if k == "properties" {
                                if let Some(target_props) = map
                                    .entry("properties".to_string())
                                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
                                    .as_object_mut()
                                {
                                    if let Some(source_props) = v.as_object() {
                                        for (pk, pv) in source_props {
                                            target_props
                                                .entry(pk.clone())
                                                .or_insert_with(|| pv.clone());
                                        }
                                    }
                                }
                            } else if k == "required" {
                                if let Some(target_req) = map
                                    .entry("required".to_string())
                                    .or_insert_with(|| Value::Array(Vec::new()))
                                    .as_array_mut()
                                {
                                    if let Some(source_req) = v.as_array() {
                                        for rv in source_req {
                                            if !target_req.contains(rv) {
                                                target_req.push(rv.clone());
                                            }
                                        }
                                    }
                                }
                            } else if !map.contains_key(&k) {
                                map.insert(k, v);
                            }
                        }
                    }
                }
            }

            // 3. [SAFETY] CheckCurrentObjectYesNo为 JSON Schema Node
            // OnlyWhenObjectlook like Schema (Packet含 type, properties, items, enum, anyOf 等) 时，才ExecutewhitelistFilter。
            // Else，If它YesoneNormal的 Value (如 request.rs in functionCall Object)，Apply radical directlyFilterwill destroyStruct。
            let looks_like_schema = map.contains_key("type")
                || map.contains_key("properties")
                || map.contains_key("items")
                || map.contains_key("enum")
                || map.contains_key("anyOf")
                || map.contains_key("oneOf")
                || map.contains_key("allOf");

            if looks_like_schema {
                // 4. [ROBUST] constraint migration：being whitelistedFilter前，将Checksumitem to description Hint
                let mut hints = Vec::new();
                let constraints = [
                    ("minLength", "minLen"),
                    ("maxLength", "maxLen"),
                    ("pattern", "pattern"),
                    ("minimum", "min"),
                    ("maximum", "max"),
                    ("multipleOf", "multipleOf"),
                    ("exclusiveMinimum", "exclMin"),
                    ("exclusiveMaximum", "exclMax"),
                    ("minItems", "minItems"),
                    ("maxItems", "maxItems"),
                    ("propertyNames", "propertyNames"),
                    ("format", "format"),
                ];
                for (field, label) in constraints {
                    if let Some(val) = map.get(field) {
                        if !val.is_null() {
                            let val_str = if let Some(s) = val.as_str() {
                                s.to_string()
                            } else {
                                val.to_string()
                            };
                            hints.push(format!("{}: {}", label, val_str));
                        }
                    }
                }
                if !hints.is_empty() {
                    let suffix = format!(" [Constraint: {}]", hints.join(", "));
                    let desc_val = map
                        .entry("description".to_string())
                        .or_insert_with(|| Value::String("".to_string()));
                    if let Value::String(s) = desc_val {
                        if !s.contains(&suffix) {
                            s.push_str(&suffix);
                        }
                    }
                }

                // 5. [CRITICAL] whitelistFilter：Thoroughly physicalRemove Gemini Not supported的Content，prevent 400 Error
                let allowed_fields = std::collections::HashSet::from([
                    "type",
                    "description",
                    "properties",
                    "required",
                    "items",
                    "enum",
                    "title",
                ]);
                let keys_to_remove: Vec<String> = map
                    .keys()
                    .filter(|k| !allowed_fields.contains(k.as_str()))
                    .cloned()
                    .collect();
                for k in keys_to_remove {
                    map.remove(&k);
                }

                // 6. [SAFETY] HandleEmpty Object
                if map.get("type").and_then(|t| t.as_str()) == Some("object") {
                    let has_props = map
                        .get("properties")
                        .and_then(|p| p.as_object())
                        .map(|o| !o.is_empty())
                        .unwrap_or(false);
                    if !has_props {
                        map.insert("properties".to_string(), serde_json::json!({
                            "reason": { "type": "string", "description": "Reason for calling this tool" }
                        }));
                        map.insert("required".to_string(), serde_json::json!(["reason"]));
                    }
                }

                // 7. [SAFETY] Required FieldAlignment
                let valid_prop_keys: Option<std::collections::HashSet<String>> = map
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .map(|obj| obj.keys().cloned().collect());

                if let Some(required_val) = map.get_mut("required") {
                    if let Some(req_arr) = required_val.as_array_mut() {
                        if let Some(keys) = &valid_prop_keys {
                            req_arr
                                .retain(|k| k.as_str().map(|s| keys.contains(s)).unwrap_or(false));
                        } else {
                            req_arr.clear();
                        }
                    }
                }

                // 8. Handle type Field
                if let Some(type_val) = map.get_mut("type") {
                    let mut selected_type = None;
                    match type_val {
                        Value::String(s) => {
                            let lower = s.to_lowercase();
                            if lower == "null" {
                                is_effectively_nullable = true;
                            } else {
                                selected_type = Some(lower);
                            }
                        }
                        Value::Array(arr) => {
                            for item in arr {
                                if let Value::String(s) = item {
                                    let lower = s.to_lowercase();
                                    if lower == "null" {
                                        is_effectively_nullable = true;
                                    } else if selected_type.is_none() {
                                        selected_type = Some(lower);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    *type_val =
                        Value::String(selected_type.unwrap_or_else(|| "string".to_string()));
                }

                if is_effectively_nullable {
                    let desc_val = map
                        .entry("description".to_string())
                        .or_insert_with(|| Value::String("".to_string()));
                    if let Value::String(s) = desc_val {
                        if !s.contains("nullable") {
                            if !s.is_empty() {
                                s.push(' ');
                            }
                            s.push_str("(nullable)");
                        }
                    }
                }

                // 9. Enum ValueForce conversion to string
                if let Some(Value::Array(arr)) = map.get_mut("enum") {
                    for item in arr {
                        if !item.is_string() {
                            *item = Value::String(if item.is_null() {
                                "null".to_string()
                            } else {
                                item.to_string()
                            });
                        }
                    }
                }
            }
        }
        Value::Array(arr) => {
            // [FIX] recursive cleanupArrayinEachelement
            // This ensuresAllArrayType的Value（Packetincluding but not limited to anyOf、oneOf、items、enum 等）will beRecursive processing
            for item in arr.iter_mut() {
                clean_json_schema_recursive(item);
            }
        }
        _ => {}
    }

    is_effectively_nullable
}

/// [NEW] Merge allOf ArrayinAll子 Schema
fn merge_all_of(map: &mut serde_json::Map<String, Value>) {
    if let Some(Value::Array(all_of)) = map.remove("allOf") {
        let mut merged_properties = serde_json::Map::new();
        let mut merged_required = std::collections::HashSet::new();
        let mut other_fields = serde_json::Map::new();

        for sub_schema in all_of {
            if let Value::Object(sub_map) = sub_schema {
                // MergeProperty
                if let Some(Value::Object(props)) = sub_map.get("properties") {
                    for (k, v) in props {
                        merged_properties.insert(k.clone(), v.clone());
                    }
                }

                // Merge required
                if let Some(Value::Array(reqs)) = sub_map.get("required") {
                    for req in reqs {
                        if let Some(s) = req.as_str() {
                            merged_required.insert(s.to_string());
                        }
                    }
                }

                // Mergethe remainingField (The first one to appear wins)
                for (k, v) in sub_map {
                    if k != "properties"
                        && k != "required"
                        && k != "allOf"
                        && !other_fields.contains_key(&k)
                    {
                        other_fields.insert(k, v);
                    }
                }
            }
        }

        // applicationMergelaterField
        for (k, v) in other_fields {
            if !map.contains_key(&k) {
                map.insert(k, v);
            }
        }

        if !merged_properties.is_empty() {
            let existing_props = map
                .entry("properties".to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Value::Object(existing_map) = existing_props {
                for (k, v) in merged_properties {
                    existing_map.entry(k).or_insert(v);
                }
            }
        }

        if !merged_required.is_empty() {
            let existing_reqs = map
                .entry("required".to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(req_arr) = existing_reqs {
                let mut current_reqs: std::collections::HashSet<String> = req_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                for req in merged_required {
                    if current_reqs.insert(req.clone()) {
                        req_arr.push(Value::String(req));
                    }
                }
            }
        }
    }
}

/// [NEW] calculate Schema branchedComplexdegree score (used for anyOf/oneOf Select the best)
/// Scoring criteria: Object (3) > Array (2) > Scalar (1) > Null (0)
fn score_schema_option(val: &Value) -> i32 {
    if let Value::Object(obj) = val {
        if obj.contains_key("properties")
            || obj.get("type").and_then(|t| t.as_str()) == Some("object")
        {
            return 3;
        }
        if obj.contains_key("items") || obj.get("type").and_then(|t| t.as_str()) == Some("array") {
            return 2;
        }
        if let Some(type_str) = obj.get("type").and_then(|t| t.as_str()) {
            if type_str != "null" {
                return 1;
            }
        }
    }
    0
}

/// [NEW] 从 anyOf/oneOf Union typeArrayChoose the best non- null Schema branch
fn extract_best_schema_from_union(union_array: &Vec<Value>) -> Option<Value> {
    let mut best_option: Option<&Value> = None;
    let mut best_score = -1;

    for item in union_array {
        let score = score_schema_option(item);
        if score > best_score {
            best_score = score;
            best_option = Some(item);
        }
    }

    best_option.cloned()
}

/// CorrectionToolcallParameter的Type，make it conform to schema definition
///
/// according to schema in type definition，automaticConvertParameterValue的Type：
/// - "123" → 123 (string → number/integer)
/// - "true" → true (string → boolean)
/// - 123 → "123" (number → string)
///
/// # Arguments
/// * `args` - ToolcalledParameterObject (will be put in placeModified)
/// * `schema` - Tool的Parameter schema definition (generallyYes parameters Object)
pub fn fix_tool_call_args(args: &mut Value, schema: &Value) {
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        if let Some(args_obj) = args.as_object_mut() {
            for (key, value) in args_obj.iter_mut() {
                if let Some(prop_schema) = properties.get(key) {
                    fix_single_arg_recursive(value, prop_schema);
                }
            }
        }
    }
}

/// Recursively fix a singleParameter的Type
fn fix_single_arg_recursive(value: &mut Value, schema: &Value) {
    // 1. HandleNestedObject (properties)
    if let Some(nested_props) = schema.get("properties").and_then(|p| p.as_object()) {
        if let Some(value_obj) = value.as_object_mut() {
            for (key, nested_value) in value_obj.iter_mut() {
                if let Some(nested_schema) = nested_props.get(key) {
                    fix_single_arg_recursive(nested_value, nested_schema);
                }
            }
        }
        return;
    }

    // 2. HandleArray (items)
    let schema_type = schema
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_lowercase();
    if schema_type == "array" {
        if let Some(items_schema) = schema.get("items") {
            if let Some(arr) = value.as_array_mut() {
                for item in arr {
                    fix_single_arg_recursive(item, items_schema);
                }
            }
        }
        return;
    }

    // 3. HandleBaseTypeCorrection
    match schema_type.as_str() {
        "number" | "integer" => {
            // string → number
            if let Some(s) = value.as_str() {
                // [SAFETY] Protect with leading zerosVersionnumber or code (如 "01", "007")，should not be converted to a number
                if s.starts_with('0') && s.len() > 1 && !s.starts_with("0.") {
                    return;
                }

                // priorityTryingParseis an integer
                if let Ok(i) = s.parse::<i64>() {
                    *value = Value::Number(serde_json::Number::from(i));
                } else if let Ok(f) = s.parse::<f64>() {
                    if let Some(n) = serde_json::Number::from_f64(f) {
                        *value = Value::Number(n);
                    }
                }
            }
        }
        "boolean" => {
            // string → Boolean
            if let Some(s) = value.as_str() {
                match s.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => *value = Value::Bool(true),
                    "false" | "0" | "no" | "off" => *value = Value::Bool(false),
                    _ => {}
                }
            } else if let Some(n) = value.as_i64() {
                // number 1/0 -> Boolean
                if n == 1 {
                    *value = Value::Bool(true);
                } else if n == 0 {
                    *value = Value::Bool(false);
                }
            }
        }
        "string" => {
            // non-string → string (preventClientMisrepresenting numbers to textField)
            if !value.is_string() && !value.is_null() && !value.is_object() && !value.is_array() {
                *value = Value::String(value.to_string());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_clean_json_schema_draft_2020_12() {
        let mut schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "minLength": 1,
                    "format": "city"
                },
                // simulationPropertyname conflict：pattern Yesone Object Property，should not beRemove
                "pattern": {
                    "type": "object",
                    "properties": {
                        "regex": { "type": "string", "pattern": "^[a-z]+$" }
                    }
                },
                "unit": {
                    "type": ["string", "null"],
                    "default": "celsius"
                }
            },
            "required": ["location"]
        });

        clean_json_schema(&mut schema);

        // 1. ValidateTypekeep lower case
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["location"]["type"], "string");

        // 2. ValidatestandardField被Removeand converted to description (Robust Constraint Migration)
        assert!(schema["properties"]["location"].get("minLength").is_none());
        assert!(schema["properties"]["location"].get("format").is_none());
        assert!(schema["properties"]["location"]["description"]
            .as_str()
            .unwrap()
            .contains("[Constraint: minLen: 1, format: city]"));

        // 3. Validatenamed "pattern" 的PropertyNot deleted by mistake
        assert!(schema["properties"].get("pattern").is_some());
        assert_eq!(schema["properties"]["pattern"]["type"], "object");

        // 4. ValidateInside的 pattern Validation field被Removeand converted to description
        assert!(schema["properties"]["pattern"]["properties"]["regex"]
            .get("pattern")
            .is_none());
        assert!(
            schema["properties"]["pattern"]["properties"]["regex"]["description"]
                .as_str()
                .unwrap()
                .contains("[Constraint: pattern: ^[a-z]+$]")
        );

        // 5. ValidateUnion type被Fallbackfor singleType (Protobuf Compatible性)
        assert_eq!(schema["properties"]["unit"]["type"], "string");

        // 6. Validate元DataField被Remove
        assert!(schema.get("$schema").is_none());
    }

    #[test]
    fn test_type_fallback() {
        // Test ["string", "null"] -> "string"
        let mut s1 = json!({"type": ["string", "null"]});
        clean_json_schema(&mut s1);
        assert_eq!(s1["type"], "string");

        // Test ["integer", "null"] -> "integer" (and lowercase check if needed, though usually integer)
        let mut s2 = json!({"type": ["integer", "null"]});
        clean_json_schema(&mut s2);
        assert_eq!(s2["type"], "integer");
    }

    #[test]
    fn test_flatten_refs() {
        let mut schema = json!({
            "$defs": {
                "Address": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    }
                }
            },
            "properties": {
                "home": { "$ref": "#/$defs/Address" }
            }
        });

        clean_json_schema(&mut schema);

        // ValidateReference被Expand且TypeConvert to lowercase
        assert_eq!(schema["properties"]["home"]["type"], "object");
        assert_eq!(
            schema["properties"]["home"]["properties"]["city"]["type"],
            "string"
        );
    }

    #[test]
    fn test_clean_json_schema_missing_required() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "existing_prop": { "type": "string" }
            },
            "required": ["existing_prop", "missing_prop"]
        });

        clean_json_schema(&mut schema);

        // Validate missing_prop from required 中Remove
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str().unwrap(), "existing_prop");
    }

    // [NEW TEST] Validate anyOf Typeextract
    #[test]
    fn test_anyof_type_extraction() {
        // Test FastMCP Stylish Optional[str] schema
        let mut schema = json!({
            "type": "object",
            "properties": {
                "testo": {
                    "anyOf": [
                        {"type": "string"},
                        {"type": "null"}
                    ],
                    "default": null,
                    "title": "Testo"
                },
                "importo": {
                    "anyOf": [
                        {"type": "number"},
                        {"type": "null"}
                    ],
                    "default": null,
                    "title": "Importo"
                },
                "attivo": {
                    "type": "boolean",
                    "title": "Attivo"
                }
            }
        });

        clean_json_schema(&mut schema);

        // Validate anyOf 被Remove
        assert!(schema["properties"]["testo"].get("anyOf").is_none());
        assert!(schema["properties"]["importo"].get("anyOf").is_none());

        // Validate type was extracted correctly
        assert_eq!(schema["properties"]["testo"]["type"], "string");
        assert_eq!(schema["properties"]["importo"]["type"], "number");
        assert_eq!(schema["properties"]["attivo"]["type"], "boolean");

        // Validate default 被Remove (Outside the whitelist)
        assert!(schema["properties"]["testo"].get("default").is_none());
    }

    // [NEW TEST] Validate oneOf Typeextract
    #[test]
    fn test_oneof_type_extraction() {
        let mut schema = json!({
            "properties": {
                "value": {
                    "oneOf": [
                        {"type": "integer"},
                        {"type": "null"}
                    ]
                }
            }
        });

        clean_json_schema(&mut schema);

        assert!(schema["properties"]["value"].get("oneOf").is_none());
        assert_eq!(schema["properties"]["value"]["type"], "integer");
    }

    // [NEW TEST] ValidateAlready have type Not covered
    #[test]
    fn test_existing_type_preserved() {
        let mut schema = json!({
            "properties": {
                "name": {
                    "type": "string",
                    "anyOf": [
                        {"type": "number"}
                    ]
                }
            }
        });

        clean_json_schema(&mut schema);

        // type Already exists，should not be anyOf inTypecover
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert!(schema["properties"]["name"].get("anyOf").is_none());
    }

    // [NEW TEST] Validate Issue #815: anyOf InsidePropertynot lost
    #[test]
    fn test_issue_815_anyof_properties_preserved() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "anyOf": [
                        {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "recursive": { "type": "boolean" }
                            },
                            "required": ["path"]
                        },
                        { "type": "null" }
                    ]
                }
            }
        });

        clean_json_schema(&mut schema);

        let config = &schema["properties"]["config"];

        // 1. ValidateTypeextracted
        assert_eq!(config["type"], "object");

        // 2. Validate anyOf Inside的 properties 被MergeComing up
        assert!(config.get("properties").is_some());
        assert_eq!(config["properties"]["path"]["type"], "string");
        assert_eq!(config["properties"]["recursive"]["type"], "boolean");

        // 3. Validate required 被MergeComing up
        let req = config["required"].as_array().unwrap();
        assert!(req.iter().any(|v| v == "path"));

        // 4. Validate anyOf FielditselfRemove
        assert!(config.get("anyOf").is_none());

        // 5. ValidateNoneBecause“Empty”And inject reason (Becausewe retainedProperty)
        assert!(config["properties"].get("reason").is_none());
    }

    // [NEW TEST] ValidateSafetyCheck：should notHandle非 Schema Object（ProtectToolcall）
    #[test]
    fn test_clean_json_schema_on_non_schema_object() {
        // simulation request.rs 中ConvertHalf of it functionCall Object
        let mut tool_call = json!({
            "functionCall": {
                "name": "local_shell_call",
                "args": { "command": ["ls"] },
                "id": "call_123"
            }
        });

        // Call cleaning logic
        clean_json_schema(&mut tool_call);

        // Validate：These are very Schema Fieldshould not beRemove（BecauseDoes not meet looks_like_schema determination）
        let fc = &tool_call["functionCall"];
        assert_eq!(fc["name"], "local_shell_call");
        assert_eq!(fc["args"]["command"][0], "ls");
        assert_eq!(fc["id"], "call_123");
    }

    // [NEW TEST] Validate Nullable Handle
    #[test]
    fn test_nullable_handling_with_description() {
        let mut schema = json!({
            "type": ["string", "null"],
            "description": "User name"
        });

        clean_json_schema(&mut schema);

        // Validate type 被Fallback，and the description is appended (nullable)
        assert_eq!(schema["type"], "string");
        assert!(schema["description"]
            .as_str()
            .unwrap()
            .contains("User name"));
        assert!(schema["description"]
            .as_str()
            .unwrap()
            .contains("(nullable)"));
    }

    // [NEW TEST] Validate anyOf Inside的 propertyNames 被Remove
    #[test]
    fn test_clean_anyof_with_propertynames() {
        let mut schema = json!({
            "properties": {
                "config": {
                    "anyOf": [
                        {
                            "type": "object",
                            "propertyNames": {"pattern": "^[a-z]+$"},
                            "properties": {
                                "key": {"type": "string"}
                            }
                        },
                        {"type": "null"}
                    ]
                }
            }
        });

        clean_json_schema(&mut schema);

        // Validate anyOf 被Remove（has beenMerge）
        let config = &schema["properties"]["config"];
        assert!(config.get("anyOf").is_none());

        // Validate propertyNames 被Remove
        assert!(config.get("propertyNames").is_none());

        // ValidateMergelater properties exist andNone propertyNames
        assert!(config.get("properties").is_some());
        assert_eq!(config["properties"]["key"]["type"], "string");
    }

    // [NEW TEST] Validate items Arrayin const 被Remove
    #[test]
    fn test_clean_items_array_with_const() {
        let mut schema = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "status": {
                        "const": "active",
                        "type": "string"
                    }
                }
            }
        });

        clean_json_schema(&mut schema);

        // Validate const 被Remove
        let status = &schema["items"]["properties"]["status"];
        assert!(status.get("const").is_none());

        // Validate type still exists
        assert_eq!(status["type"], "string");
    }

    // [NEW TEST] ValidateMultiple levels of nestingArrayclean up
    #[test]
    fn test_deep_nested_array_cleaning() {
        let mut schema = json!({
            "properties": {
                "data": {
                    "anyOf": [
                        {
                            "type": "array",
                            "items": {
                                "anyOf": [
                                    {
                                        "type": "object",
                                        "propertyNames": {"maxLength": 10},
                                        "const": "test",
                                        "properties": {
                                            "name": {"type": "string"}
                                        }
                                    },
                                    {"type": "null"}
                                ]
                            }
                        }
                    ]
                }
            }
        });

        clean_json_schema(&mut schema);

        // ValidateDeeply nested illegalFieldAll wereRemove
        let data = &schema["properties"]["data"];

        // anyOf Should被MergeRemove
        assert!(data.get("anyOf").is_none());

        // ValidateNone propertyNames 和 const Escape to the top
        assert!(data.get("propertyNames").is_none());
        assert!(data.get("const").is_none());

        // ValidateStructcorrectly retained
        assert_eq!(data["type"], "array");
        if let Some(items) = data.get("items") {
            // items Inside的 anyOf 也Should被Merge
            assert!(items.get("anyOf").is_none());
            assert!(items.get("propertyNames").is_none());
            assert!(items.get("const").is_none());
        }
    }

    #[test]
    fn test_fix_tool_call_args() {
        let mut args = serde_json::json!({
            "port": "8080",
            "enabled": "true",
            "timeout": "5.5",
            "metadata": {
                "retry": "3"
            },
            "tags": ["1", "2"]
        });

        let schema = serde_json::json!({
            "properties": {
                "port": { "type": "integer" },
                "enabled": { "type": "boolean" },
                "timeout": { "type": "number" },
                "metadata": {
                    "type": "object",
                    "properties": {
                        "retry": { "type": "integer" }
                    }
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "integer" }
                }
            }
        });

        fix_tool_call_args(&mut args, &schema);

        assert_eq!(args["port"], 8080);
        assert_eq!(args["enabled"], true);
        assert_eq!(args["timeout"], 5.5);
        assert_eq!(args["metadata"]["retry"], 3);
        assert_eq!(args["tags"], serde_json::json!([1, 2]));
    }

    #[test]
    fn test_fix_tool_call_args_protection() {
        let mut args = serde_json::json!({
            "version": "01.0",
            "code": "007"
        });

        let schema = serde_json::json!({
            "properties": {
                "version": { "type": "number" },
                "code": { "type": "integer" }
            }
        });

        fix_tool_call_args(&mut args, &schema);

        // Strings should be preserved to avoid breaking semantics
        assert_eq!(args["version"], "01.0");
        assert_eq!(args["code"], "007");
    }

    // [NEW TEST #952] ValidateNestedHierarchy的 $defs can be collected correctly andExpand
    #[test]
    fn test_nested_defs_flattening() {
        // MCP Tooloften $defs nested in properties Inside，rather than rootHierarchy
        let mut schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "$defs": {
                        "Address": {
                            "type": "object",
                            "properties": {
                                "city": { "type": "string" },
                                "zip": { "type": "string" }
                            }
                        }
                    },
                    "type": "object",
                    "properties": {
                        "home": { "$ref": "#/$defs/Address" },
                        "work": { "$ref": "#/$defs/Address" }
                    }
                }
            }
        });

        clean_json_schema(&mut schema);

        // ValidateNested $ref be correctParse
        let home = &schema["properties"]["config"]["properties"]["home"];
        assert_eq!(
            home["type"], "object",
            "home should have type 'object' from resolved $ref"
        );
        assert_eq!(
            home["properties"]["city"]["type"], "string",
            "home.properties.city should exist from resolved Address"
        );

        // ValidateNoneresidual $ref
        assert!(
            home.get("$ref").is_none(),
            "home should not have orphan $ref"
        );

        // Validate work also correctParse
        let work = &schema["properties"]["config"]["properties"]["work"];
        assert_eq!(work["type"], "object");
        assert!(work.get("$ref").is_none());
    }

    // [NEW TEST #952] ValidateUnable to parse的 $ref be gracedFallback
    #[test]
    fn test_unresolved_ref_fallback() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "external": { "$ref": "https://example.com/schemas/External.json" },
                "missing": { "$ref": "#/$defs/NonExistent" }
            }
        });

        clean_json_schema(&mut schema);

        // ValidateOutsideReference被Fallback为 string Type
        let external = &schema["properties"]["external"];
        assert_eq!(
            external["type"], "string",
            "unresolved external $ref should fallback to string"
        );
        assert!(
            external["description"]
                .as_str()
                .unwrap()
                .contains("Unresolved $ref"),
            "description should contain unresolved $ref hint"
        );

        // ValidateInsideMissingReferencealsoFallback
        let missing = &schema["properties"]["missing"];
        assert_eq!(missing["type"], "string");
        assert!(missing["description"]
            .as_str()
            .unwrap()
            .contains("NonExistent"));
    }

    // [NEW TEST #952] ValidateDeeply nested multi-level $defs can be collected
    #[test]
    fn test_deeply_nested_multi_level_defs() {
        let mut schema = json!({
            "type": "object",
            "$defs": {
                "RootDef": { "type": "integer" }
            },
            "properties": {
                "level1": {
                    "type": "object",
                    "$defs": {
                        "Level1Def": { "type": "boolean" }
                    },
                    "properties": {
                        "level2": {
                            "type": "object",
                            "$defs": {
                                "Level2Def": { "type": "number" }
                            },
                            "properties": {
                                "useRoot": { "$ref": "#/$defs/RootDef" },
                                "useLevel1": { "$ref": "#/$defs/Level1Def" },
                                "useLevel2": { "$ref": "#/$defs/Level2Def" }
                            }
                        }
                    }
                }
            }
        });

        clean_json_schema(&mut schema);

        let level2_props = &schema["properties"]["level1"]["properties"]["level2"]["properties"];

        // ValidateAllHierarchy的 $defs are correctParse
        assert_eq!(
            level2_props["useRoot"]["type"], "integer",
            "RootDef should resolve"
        );
        assert_eq!(
            level2_props["useLevel1"]["type"], "boolean",
            "Level1Def should resolve"
        );
        assert_eq!(
            level2_props["useLevel2"]["type"], "number",
            "Level2Def should resolve"
        );

        // ValidateNoneResidue $ref
        assert!(level2_props["useRoot"].get("$ref").is_none());
        assert!(level2_props["useLevel1"].get("$ref").is_none());
        assert!(level2_props["useLevel2"].get("$ref").is_none());
    }
}
