use country_lookup_native::{data, format, js};
use serde_json::{json, Value};
#[test]
fn actual_source_oracle_full_text_raw_keywords_and_metadata() {
    let oracle: Value = serde_json::from_str(include_str!("../qa/oracle.json")).unwrap();
    let names: format::Names = oracle["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v[0].as_str().unwrap().into(), v[1].as_str().unwrap().into()))
        .collect();
    for (i, case) in oracle["cases"].as_array().unwrap().iter().enumerate() {
        let c = js::parse(case["input"].as_str().unwrap().as_bytes()).unwrap();
        assert_eq!(
            format::text(&c, &names),
            case["text"].as_str().unwrap(),
            "text case {i}"
        );
        assert_eq!(
            js::stringify(&c),
            case["raw"].as_str().unwrap(),
            "raw case {i}"
        );
        assert_eq!(
            format::common(&c),
            case["common"].as_str().unwrap(),
            "common {i}"
        );
        assert_eq!(
            json!(format::keywords(&c)),
            case["keywords"],
            "keywords {i}"
        );
        let m = format::metadata(&c, &names);
        let actual = m["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                let mut v = json!({"type":n["type"]});
                for k in ["title", "text", "target"] {
                    if let Some(x) = n["props"].get(k) {
                        v[k] = x.clone()
                    }
                }
                if n["type"].as_str().unwrap().ends_with(".TagList") {
                    v["items"] = json!(n["children"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c["props"]["text"].clone())
                        .collect::<Vec<_>>())
                }
                v
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(actual), case["metadata"], "metadata {i}");
    }
}
#[test]
fn trim_filter_last_map_and_complete_keyword_search() {
    let a = json!([{"names":{"common":"old"},"codes":{"alpha_3":" DUP "}},{"names":{"common":"new"},"codes":{"alpha_3":"DUP"},"classification":{"disputed":true}},{"names":{"common":"none"},"codes":{"alpha_3":"  "}}]);
    let visible = format::filter(a.as_array().unwrap(), true);
    assert_eq!(visible.len(), 2);
    assert_eq!(format::names(&visible)["DUP"], "new");
    assert_eq!(format::filter(a.as_array().unwrap(), false).len(), 3);
    let whitespace = json!([{"names":{"common":"BOM"},"codes":{"alpha_3":"\u{feff}"}},{"names":{"common":"NEL"},"codes":{"alpha_3":"\u{85}"}}]);
    let visible = format::filter(whitespace.as_array().unwrap(), true);
    assert_eq!(visible.len(), 1);
    assert_eq!(format::common(&visible[0]), "NEL");
    assert_eq!(format::names(&visible)["\u{85}"], "NEL");
    let oracle: Value = serde_json::from_str(include_str!("../qa/oracle.json")).unwrap();
    let c = js::parse(oracle["cases"][0]["input"].as_str().unwrap().as_bytes()).unwrap();
    for k in format::keywords(&c) {
        assert!(format::matches(&c, &k), "keyword missing")
    }
    assert!(!format::matches(&c, "nonexistent123"));
}
#[test]
fn finite_unicode_and_url_security() {
    assert!(js::parse(br#"{"x":"\ud800"}"#).is_err());
    assert!(js::parse(br#"{"x":1e400}"#).is_err());
    assert!(js::parse(br#"{"x":"\ud83d\ude00"}"#).is_ok());
    for u in [
        "file:///private/x",
        "javascript:alert(1)",
        "https://localhost/x",
        "https://127.0.0.1/x",
        "https://[::1]/",
        "https://user:pass@www.gov.kz",
        "https://www.gov.kz/\n",
        "https://evil.local/",
        "https://www.gov.kz:8888/",
        "https://2130706433/",
    ] {
        assert!(data::public_url(u).is_none(), "URL admitted")
    }
    for u in [
        "https://evil.org/x.png",
        "http://flagcdn.com/x.png",
        "https://flagcdn.com/x.svg",
        "https://flagcdn.com/x.png?raycast-width=320",
        "data:image/svg+xml,test",
    ] {
        assert!(data::flag_url(u).is_none())
    }
    assert!(data::flag_url("https://flagcdn.com/w320/kz.png").is_some());
    let c = json!({"names":{"common":"<button>[x](file:///x)</button>"},"flag":{"description":"<script>x</script>"}});
    let m = format::markdown(&c);
    assert!(!m.contains('<'));
    assert!(!m.contains("file:"));
    assert!(!m.contains("[x]"));
}
