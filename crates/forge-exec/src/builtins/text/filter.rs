/// Minimal jq-style filter engine for structured data.
/// Operates on `serde_json::Value`; callers convert YAML/TOML to/from JSON.
use serde_json::Value;

/// Evaluate a filter string against an input value.
/// Returns the list of output values (a filter may produce multiple outputs).
///
/// # Errors
/// Returns a `String` describing the error if the filter is invalid or evaluation fails.
#[allow(clippy::needless_pass_by_value)]
pub fn eval_filter(
    filter: &str,
    input: Value,
    bindings: &[(String, Value)],
) -> Result<Vec<Value>, String> {
    let filter = filter.trim();
    eval(filter, &input, bindings)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn eval(expr: &str, input: &Value, bindings: &[(String, Value)]) -> Result<Vec<Value>, String> {
    let expr = expr.trim();

    // Pipe: split on top-level `|`
    if let Some(pipe_pos) = find_top_level(expr, '|') {
        let left = expr[..pipe_pos].trim();
        let right = expr[pipe_pos + 1..].trim();
        let left_results = eval(left, input, bindings)?;
        let mut out = Vec::new();
        for v in left_results {
            out.extend(eval(right, &v, bindings)?);
        }
        return Ok(out);
    }

    // Comma: top-level comma → multiple outputs
    if let Some(comma_pos) = find_top_level(expr, ',') {
        let left = expr[..comma_pos].trim();
        let right = expr[comma_pos + 1..].trim();
        let mut out = eval(left, input, bindings)?;
        out.extend(eval(right, input, bindings)?);
        return Ok(out);
    }

    // Parentheses grouping: (expr)
    if expr.starts_with('(') && expr.ends_with(')') {
        return eval(&expr[1..expr.len() - 1], input, bindings);
    }

    // Identity
    if expr == "." {
        return Ok(vec![input.clone()]);
    }

    // null literal
    if expr == "null" {
        return Ok(vec![Value::Null]);
    }

    // true / false literals
    if expr == "true" {
        return Ok(vec![Value::Bool(true)]);
    }
    if expr == "false" {
        return Ok(vec![Value::Bool(false)]);
    }

    // Number literal
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(vec![Value::from(n)]);
    }

    // String literal
    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        let s = &expr[1..expr.len() - 1];
        return Ok(vec![Value::String(s.to_string())]);
    }

    // $name binding
    if let Some(name) = expr.strip_prefix('$') {
        if let Some((_, v)) = bindings.iter().find(|(k, _)| k == name) {
            return Ok(vec![v.clone()]);
        }
        return Ok(vec![Value::Null]);
    }

    // .[] — iterate
    if expr == ".[]" {
        return Ok(match input {
            Value::Array(arr) => arr.clone(),
            Value::Object(map) => map.values().cloned().collect(),
            _ => vec![],
        });
    }

    // .[N] — array index
    if expr.starts_with(".[") && expr.ends_with(']') {
        let inner = &expr[2..expr.len() - 1].trim();
        if let Ok(idx) = inner.parse::<i64>() {
            return Ok(match input {
                Value::Array(arr) => {
                    let i = if idx < 0 { arr.len() as i64 + idx } else { idx } as usize;
                    vec![arr.get(i).cloned().unwrap_or(Value::Null)]
                }
                _ => vec![Value::Null],
            });
        }
        // .[N:M] slice
        if let Some(colon) = inner.find(':') {
            let start_s = inner[..colon].trim();
            let end_s = inner[colon + 1..].trim();
            return Ok(match input {
                Value::Array(arr) => {
                    let len = arr.len() as i64;
                    let s = parse_opt_index(start_s, 0, len) as usize;
                    let e = parse_opt_index(end_s, len, len) as usize;
                    vec![Value::Array(
                        arr[s.min(arr.len())..e.min(arr.len())].to_vec(),
                    )]
                }
                Value::String(st) => {
                    let chars: Vec<char> = st.chars().collect();
                    let len = chars.len() as i64;
                    let s = parse_opt_index(start_s, 0, len) as usize;
                    let e = parse_opt_index(end_s, len, len) as usize;
                    vec![Value::String(
                        chars[s.min(chars.len())..e.min(chars.len())]
                            .iter()
                            .collect(),
                    )]
                }
                _ => vec![Value::Null],
            });
        }
    }

    // .field or .field.sub or .["field"]
    if let Some(path) = parse_path(expr) {
        return Ok(vec![get_path(input, &path)]);
    }

    // Built-in functions
    if let Some(result) = eval_builtin(expr, input, bindings)? {
        return Ok(result);
    }

    // Unrecognised — return null
    Ok(vec![Value::Null])
}

fn parse_opt_index(s: &str, default: i64, len: i64) -> i64 {
    if s.is_empty() {
        return default;
    }
    let n: i64 = s.parse().unwrap_or(default);
    if n < 0 { (len + n).max(0) } else { n }
}

fn get_path(v: &Value, path: &[String]) -> Value {
    let mut cur = v.clone();
    for key in path {
        cur = match cur {
            Value::Object(ref map) => map.get(key).cloned().unwrap_or(Value::Null),
            Value::Array(ref arr) => {
                if let Ok(i) = key.parse::<usize>() {
                    arr.get(i).cloned().unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        };
    }
    cur
}

fn parse_path(expr: &str) -> Option<Vec<String>> {
    if !expr.starts_with('.') {
        return None;
    }
    let rest = &expr[1..];
    if rest.is_empty() {
        return None;
    }
    if rest.starts_with('[') {
        return None;
    }
    // Split on '.' but respect brackets
    let mut parts = Vec::new();
    let mut cur = String::new();
    for c in rest.chars() {
        if c == '.' {
            if !cur.is_empty() {
                parts.push(cur.clone());
                cur.clear();
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    if parts.is_empty() { None } else { Some(parts) }
}

#[allow(clippy::too_many_lines)]
fn eval_builtin(
    expr: &str,
    input: &Value,
    bindings: &[(String, Value)],
) -> Result<Option<Vec<Value>>, String> {
    // length
    if expr == "length" {
        let n = match input {
            Value::String(s) => s.chars().count(),
            Value::Array(a) => a.len(),
            Value::Object(o) => o.len(),
            Value::Null => 0,
            _ => 1,
        };
        return Ok(Some(vec![Value::from(n as u64)]));
    }

    // keys
    if expr == "keys" {
        let v = match input {
            Value::Object(o) => {
                let mut ks: Vec<Value> = o.keys().map(|k| Value::String(k.clone())).collect();
                ks.sort_by(|a, b| a.as_str().unwrap_or("").cmp(b.as_str().unwrap_or("")));
                Value::Array(ks)
            }
            Value::Array(a) => Value::Array((0..a.len()).map(|i| Value::from(i as u64)).collect()),
            _ => Value::Null,
        };
        return Ok(Some(vec![v]));
    }

    // values
    if expr == "values" {
        let v = match input {
            Value::Object(o) => Value::Array(o.values().cloned().collect()),
            Value::Array(a) => Value::Array(a.clone()),
            _ => Value::Null,
        };
        return Ok(Some(vec![v]));
    }

    // type
    if expr == "type" {
        let t = match input {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        return Ok(Some(vec![Value::String(t.to_string())]));
    }

    // to_entries
    if expr == "to_entries" {
        let v = match input {
            Value::Object(o) => Value::Array(
                o.iter()
                    .map(|(k, v)| {
                        let mut m = serde_json::Map::new();
                        m.insert("key".to_string(), Value::String(k.clone()));
                        m.insert("value".to_string(), v.clone());
                        Value::Object(m)
                    })
                    .collect(),
            ),
            _ => Value::Null,
        };
        return Ok(Some(vec![v]));
    }

    // from_entries
    if expr == "from_entries" {
        let v = match input {
            Value::Array(arr) => {
                let mut m = serde_json::Map::new();
                for item in arr {
                    if let Value::Object(obj) = item {
                        let key = obj
                            .get("key")
                            .or_else(|| obj.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let val = obj.get("value").cloned().unwrap_or(Value::Null);
                        m.insert(key, val);
                    }
                }
                Value::Object(m)
            }
            _ => Value::Null,
        };
        return Ok(Some(vec![v]));
    }

    // has(key)
    if let Some(inner) = expr.strip_prefix("has(").and_then(|s| s.strip_suffix(')')) {
        let key = inner.trim().trim_matches('"');
        let v = match input {
            Value::Object(o) => Value::Bool(o.contains_key(key)),
            Value::Array(a) => {
                let idx: usize = key.parse().unwrap_or(usize::MAX);
                Value::Bool(idx < a.len())
            }
            _ => Value::Bool(false),
        };
        return Ok(Some(vec![v]));
    }

    // select(expr)
    if let Some(inner) = expr
        .strip_prefix("select(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let results = eval(inner, input, bindings)?;
        let truthy = results
            .iter()
            .any(|v| !matches!(v, Value::Null | Value::Bool(false)));
        return Ok(Some(if truthy { vec![input.clone()] } else { vec![] }));
    }

    // not
    if expr == "not" {
        let v = match input {
            Value::Null | Value::Bool(false) => Value::Bool(true),
            _ => Value::Bool(false),
        };
        return Ok(Some(vec![v]));
    }

    // empty
    if expr == "empty" {
        return Ok(Some(vec![]));
    }

    // add
    if expr == "add" {
        let v = match input {
            Value::Array(arr) => arr.iter().fold(Value::Null, |acc, v| add_values(&acc, v)),
            _ => Value::Null,
        };
        return Ok(Some(vec![v]));
    }

    // first, last
    if expr == "first" {
        return Ok(Some(vec![match input {
            Value::Array(a) => a.first().cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        }]));
    }
    if expr == "last" {
        return Ok(Some(vec![match input {
            Value::Array(a) => a.last().cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        }]));
    }

    // reverse
    if expr == "reverse" {
        return Ok(Some(vec![match input {
            Value::Array(a) => {
                let mut r = a.clone();
                r.reverse();
                Value::Array(r)
            }
            Value::String(s) => Value::String(s.chars().rev().collect()),
            _ => Value::Null,
        }]));
    }

    // unique
    if expr == "unique" {
        if let Value::Array(a) = input {
            let mut seen = Vec::new();
            for v in a {
                if !seen.contains(v) {
                    seen.push(v.clone());
                }
            }
            return Ok(Some(vec![Value::Array(seen)]));
        }
    }

    // sort
    if expr == "sort" {
        if let Value::Array(a) = input {
            let mut s = a.clone();
            s.sort_by(json_cmp);
            return Ok(Some(vec![Value::Array(s)]));
        }
    }

    // flatten
    if expr == "flatten" {
        if let Value::Array(a) = input {
            let flat = flatten_array(a);
            return Ok(Some(vec![Value::Array(flat)]));
        }
    }

    // group_by(.field) — simplified
    if let Some(inner) = expr
        .strip_prefix("group_by(")
        .and_then(|s| s.strip_suffix(')'))
    {
        if let Value::Array(arr) = input {
            let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
            for item in arr {
                let key = eval(inner, item, bindings)?
                    .into_iter()
                    .next()
                    .unwrap_or(Value::Null);
                if let Some(g) = groups.iter_mut().find(|(k, _)| k == &key) {
                    g.1.push(item.clone());
                } else {
                    groups.push((key, vec![item.clone()]));
                }
            }
            return Ok(Some(vec![Value::Array(
                groups.into_iter().map(|(_, v)| Value::Array(v)).collect(),
            )]));
        }
    }

    Ok(None)
}

#[allow(clippy::many_single_char_names)]
fn add_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Null, _) => b.clone(),
        (_, Value::Null) => a.clone(),
        (Value::Number(x), Value::Number(y)) => {
            Value::from(x.as_f64().unwrap_or(0.0) + y.as_f64().unwrap_or(0.0))
        }
        (Value::String(x), Value::String(y)) => Value::String(format!("{x}{y}")),
        (Value::Array(x), Value::Array(y)) => {
            let mut r = x.clone();
            r.extend_from_slice(y);
            Value::Array(r)
        }
        (Value::Object(x), Value::Object(y)) => {
            let mut m = x.clone();
            m.extend(y.iter().map(|(k, v)| (k.clone(), v.clone())));
            Value::Object(m)
        }
        _ => Value::Null,
    }
}

fn json_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    let ta = type_order(a);
    let tb = type_order(b);
    if ta != tb {
        return ta.cmp(&tb);
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&y.as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

fn type_order(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn flatten_array(arr: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for v in arr {
        match v {
            Value::Array(inner) => out.extend(flatten_array(inner)),
            other => out.push(other.clone()),
        }
    }
    out
}

/// Find the position of `c` at the top level (not inside brackets/quotes).
fn find_top_level(s: &str, c: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if ch == '\\' && in_str {
            escape = true;
            i += 1;
            continue;
        }
        if ch == '"' {
            in_str = !in_str;
            i += 1;
            continue;
        }
        if in_str {
            i += 1;
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ if ch == c && depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}
