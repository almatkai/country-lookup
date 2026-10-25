//! Finite JSON/ECMAScript compatibility. API numbers are IEEE754 as in JSON.parse.
use serde_json::Value;
pub const DASH: &str = "—";
/// ECMAScript WhiteSpace + LineTerminator (Rust's Unicode trim differs at BOM/NEL).
pub fn trim(s: &str) -> &str {
    s.trim_matches(|c|matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
pub fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
pub fn get<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, k| v.get(*k))
}
pub fn s(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => num(n.as_f64().unwrap()),
        Value::Array(a) => a
            .iter()
            .map(|v| if v.is_null() { String::new() } else { s(v) })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}
pub fn num(x: f64) -> String {
    if x.is_nan() {
        "NaN".into()
    } else if x.is_infinite() {
        if x.is_sign_negative() {
            "-Infinity".into()
        } else {
            "Infinity".into()
        }
    } else {
        ryu_js::Buffer::new().format(x).to_owned()
    }
}
pub fn nullish(v: &Value, path: &[&str], default: &str) -> String {
    get(v, path)
        .filter(|v| !v.is_null())
        .map(s)
        .unwrap_or_else(|| default.into())
}
pub fn or(v: &Value, path: &[&str], default: &str) -> String {
    get(v, path)
        .filter(|v| truth(v))
        .map(s)
        .unwrap_or_else(|| default.into())
}
pub fn arr(v: &Value, path: &[&str]) -> Vec<Value> {
    get(v, path)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
pub fn index(k: &str) -> Option<u32> {
    let n = k.parse::<u32>().ok()?;
    (n != u32::MAX && n.to_string() == k).then_some(n)
}
pub fn entries(v: &Value) -> Vec<(&String, &Value)> {
    let Some(m) = v.as_object() else {
        return vec![];
    };
    let mut a: Vec<_> = m.iter().collect();
    a.sort_by_key(|(k, _)| index(k).map(u64::from).unwrap_or(u64::MAX));
    a
}
pub fn parse(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Country response exceeds the 8 MiB page limit".into());
    }
    let mut v:Value=serde_json::from_slice(bytes).map_err(|_|"Country response is invalid JSON (finite numbers and paired Unicode surrogates required)".to_string())?;
    fn walk(v: &mut Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
        *nodes += 1;
        if depth > 48 || *nodes > 500_000 {
            return Err("Country response exceeds the JSON complexity limit".into());
        }
        match v {
            Value::Number(n) => {
                let x = n
                    .as_f64()
                    .ok_or("Country response has an unsupported number")?;
                *n = serde_json::Number::from_f64(x)
                    .ok_or("Country response has a nonfinite number")?
            }
            Value::String(s) if s.len() > 65536 => {
                return Err("Country response has an oversized string".into())
            }
            Value::Array(a) => {
                for v in a {
                    walk(v, depth + 1, nodes)?
                }
            }
            Value::Object(m) => {
                for (k, v) in m {
                    if k.len() > 1024 {
                        return Err("Country response has an oversized key".into());
                    }
                    walk(v, depth + 1, nodes)?
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&mut v, 0, &mut 0)?;
    Ok(v)
}
pub fn stringify(v: &Value) -> String {
    fn write(v: &Value, level: usize, out: &mut String) {
        let pad = |n: usize| "  ".repeat(n);
        match v {
            Value::Number(n) => out.push_str(&num(n.as_f64().unwrap())),
            Value::Array(a) if !a.is_empty() => {
                out.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    out.push_str(&pad(level + 1));
                    write(v, level + 1, out);
                    if i + 1 < a.len() {
                        out.push(',')
                    }
                    out.push('\n')
                }
                out.push_str(&pad(level));
                out.push(']')
            }
            Value::Object(m) if !m.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in entries(v).iter().enumerate() {
                    out.push_str(&pad(level + 1));
                    out.push_str(&serde_json::to_string(k).unwrap());
                    out.push_str(": ");
                    write(v, level + 1, out);
                    if i + 1 < m.len() {
                        out.push(',')
                    }
                    out.push('\n')
                }
                out.push_str(&pad(level));
                out.push('}')
            }
            _ => out.push_str(&serde_json::to_string(v).unwrap()),
        }
    }
    let mut out = String::new();
    write(v, 0, &mut out);
    out
}
/// Intl.NumberFormat("en-US"): grouping, max 3 fraction digits, halfExpand.
/// Decimal rounding is applied to the shortest ECMAScript representation.
pub fn number(v: Option<&Value>) -> String {
    let Some(v) = v else { return DASH.into() };
    let n = match v {
        Value::Null => 0.0,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => {
            if s.trim().is_empty() {
                0.0
            } else {
                s.trim().parse().unwrap_or(f64::NAN)
            }
        }
        _ => f64::NAN,
    };
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n < 0.0 { "-∞".into() } else { "∞".into() };
    }
    let raw = num(n.abs());
    let (mant, exp) = raw
        .split_once('e')
        .map(|(a, b)| (a, b.parse::<i32>().unwrap()))
        .unwrap_or((&raw, 0));
    let digits = mant.replace('.', "");
    let before = mant.find('.').unwrap_or(mant.len()) as i32 + exp;
    let mut fixed = if before <= 0 {
        format!("0.{}{}", "0".repeat((-before) as usize), digits)
    } else if before as usize >= digits.len() {
        format!("{}{}", digits, "0".repeat(before as usize - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..before as usize],
            &digits[before as usize..]
        )
    };
    let pos = fixed.find('.').unwrap_or(fixed.len());
    if fixed.len() > pos + 4 {
        let up = fixed.as_bytes()[pos + 4] >= b'5';
        fixed.truncate(pos + 4);
        if up {
            let mut b = fixed.into_bytes();
            let mut i = b.len();
            loop {
                if i == 0 {
                    b.insert(0, b'1');
                    break;
                }
                i -= 1;
                if b[i] == b'.' {
                    continue;
                }
                if b[i] < b'9' {
                    b[i] += 1;
                    break;
                }
                b[i] = b'0';
            }
            fixed = String::from_utf8(b).unwrap()
        }
    }
    if fixed.contains('.') {
        while fixed.ends_with('0') {
            fixed.pop();
        }
        if fixed.ends_with('.') {
            fixed.pop();
        }
    }
    let (a, b) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut out = String::new();
    if n.is_sign_negative() {
        out.push('-')
    }
    for (i, c) in a.chars().enumerate() {
        if i > 0 && (a.len() - i) % 3 == 0 {
            out.push(',')
        }
        out.push(c)
    }
    if !b.is_empty() {
        out.push('.');
        out.push_str(b)
    }
    out
}
pub fn markdown_text(s: &str) -> String {
    // Numeric entities remain literal text after Markdown parsing; no raw data can create markup.
    {
        use std::fmt::Write;
        let mut out = String::new();
        for c in s.chars() {
            write!(&mut out, "&#{};", c as u32).expect("String writes do not fail");
        }
        out
    }
}
