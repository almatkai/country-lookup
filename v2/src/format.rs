use crate::js::{self, *};
use serde_json::{json, Value};
use std::collections::HashMap;
pub type Names = HashMap<String, String>;
pub fn common(c: &Value) -> String {
    nullish(
        c,
        &["names", "common"],
        &nullish(c, &["names", "official"], "Unknown"),
    )
}
fn joined(a: Vec<Value>, sep: &str, fallback: &str) -> String {
    if a.is_empty() {
        fallback.into()
    } else {
        a.iter()
            .map(|v| if v.is_null() { String::new() } else { s(v) })
            .collect::<Vec<_>>()
            .join(sep)
    }
}
fn joined_nonempty(a: Vec<Value>) -> String {
    let x = joined(a, ", ", DASH);
    if x.is_empty() {
        DASH.into()
    } else {
        x
    }
}
fn filtered(a: Vec<Value>) -> Vec<Value> {
    a.into_iter().filter(truth).collect()
}
fn mapped(c: &Value, path: &[&str], key: &str) -> Vec<Value> {
    arr(c, path).iter().map(|v| v[key].clone()).collect()
}
pub fn languages(c: &Value) -> Vec<Value> {
    filtered(mapped(c, &["languages"], "name"))
}
pub fn currencies(c: &Value) -> Vec<Value> {
    arr(c, &["currencies"])
        .iter()
        .map(|v| {
            let mut a = vec![];
            if truth(&v["name"]) {
                a.push(s(&v["name"]))
            }
            if truth(&v["symbol"]) {
                a.push(format!("({})", s(&v["symbol"])))
            }
            Value::String(a.join(" "))
        })
        .collect()
}
pub fn demonym(c: &Value) -> String {
    let Some(v) = get(c, &["demonyms"]).filter(|v| truth(v)) else {
        return DASH.into();
    };
    let p = v
        .get("eng")
        .filter(|v| !v.is_null())
        .or_else(|| entries(v).first().map(|(_, v)| *v));
    p.map(|v| or(v, &["m"], &or(v, &["f"], DASH)))
        .unwrap_or_else(|| DASH.into())
}
fn native(c: &Value) -> String {
    let Some(n) = get(c, &["names", "native"]).filter(|v| truth(v)) else {
        return DASH.into();
    };
    let mut a = Vec::new();
    for (_, v) in entries(n) {
        let n = &v["common"];
        if truth(n) && !a.contains(n) {
            a.push(n.clone())
        }
    }
    joined(a, ", ", DASH)
}
fn units(c: &Value) -> String {
    let Some(u) = get(c, &["units"]).filter(|v| truth(v)) else {
        return DASH.into();
    };
    let mut a = vec![];
    if let Some(m) = u["measurement_system"].as_str().filter(|s| !s.is_empty()) {
        let mut ch = m.chars();
        a.push(format!(
            "{}{}",
            ch.next().unwrap().to_uppercase(),
            ch.as_str()
        ));
    }
    if truth(&u["temperature_scale"]) {
        a.push(s(&u["temperature_scale"]))
    }
    if a.is_empty() {
        DASH.into()
    } else {
        a.join(" · ")
    }
}
fn day(v: Option<&Value>) -> String {
    const M: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let Some(v) = v else { return DASH.into() };
    let month = v["month"].as_f64().unwrap_or(0.0);
    if month.fract() != 0.0 || !(1.0..=12.0).contains(&month) {
        return DASH.into();
    }
    let m = M[month as usize - 1];
    if truth(&v["day"]) {
        format!("{} {m}", s(&v["day"]))
    } else {
        m.into()
    }
}
fn area(c: &Value) -> String {
    if !truth(&c["area"]) {
        return DASH.into();
    }
    let v = get(c, &["area", "kilometers"]);
    let x = match v {
        Some(Value::Null) => 0.0,
        Some(v) => v.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    let rounded = if (-0.5..0.0).contains(&x) {
        -0.0
    } else {
        let floor = x.floor();
        if x - floor >= 0.5 {
            floor + 1.0
        } else {
            floor
        }
    };
    let n = serde_json::Number::from_f64(rounded).map(Value::Number);
    format!(
        "{} km²",
        if rounded.is_nan() {
            "NaN".into()
        } else {
            number(n.as_ref())
        }
    )
}
fn number_format(c: &Value) -> String {
    let Some(f) = get(c, &["number_format"]).filter(|v| truth(v)) else {
        return DASH.into();
    };
    let t = nullish(f, &["thousands_separator"], "");
    let d = nullish(f, &["decimal_separator"], "");
    if t.is_empty() && d.is_empty() {
        DASH.into()
    } else {
        format!("1{t}234{t}567{d}89")
    }
}
pub fn memberships(c: &Value) -> Vec<Value> {
    [
        ("un", "UN"),
        ("eu", "EU"),
        ("nato", "NATO"),
        ("schengen", "Schengen"),
        ("eurozone", "Eurozone"),
        ("commonwealth", "Commonwealth"),
        ("g7", "G7"),
        ("g20", "G20"),
        ("brics", "BRICS"),
        ("oecd", "OECD"),
        ("opec", "OPEC"),
        ("asean", "ASEAN"),
        ("african_union", "African Union"),
        ("arab_league", "Arab League"),
    ]
    .iter()
    .filter(|(k, _)| truth(&c["memberships"][k]))
    .map(|(_, v)| json!(v))
    .collect()
}
#[derive(Debug, Clone)]
pub enum Field {
    Label(&'static str, String),
    Tags(&'static str, Vec<Value>),
    Separator,
    Link(&'static str, String),
}
pub fn fields(c: &Value, names: &Names) -> Vec<Field> {
    use Field::*;
    let mut f = vec![
        Label("Official Name", nullish(c, &["names", "official"], DASH)),
        Label("Native Name", native(c)),
        Label("Country Code", or(c, &["codes", "alpha_2"], DASH)),
        Label("Alpha-3 Code", or(c, &["codes", "alpha_3"], DASH)),
        Label("Numeric Code", or(c, &["codes", "ccn3"], DASH)),
    ];
    for (k, t) in [("cioc", "IOC Code"), ("fifa", "FIFA Code")] {
        if truth(&c["codes"][k]) {
            f.push(Label(t, s(&c["codes"][k])))
        }
    }
    f.extend([
        Separator,
        Label(
            "Capital",
            joined(filtered(mapped(c, &["capitals"], "name")), ", ", DASH),
        ),
        Label(
            "Region",
            joined(
                filtered(vec![c["region"].clone(), c["subregion"].clone()]),
                " · ",
                DASH,
            ),
        ),
        Label("Continent", joined(arr(c, &["continents"]), ", ", DASH)),
        Label("Population", number(c.get("population"))),
        Label("Area", area(c)),
        Label(
            "Landlocked",
            c.get("landlocked")
                .map(|v| if truth(v) { "Yes" } else { "No" })
                .unwrap_or(DASH)
                .into(),
        ),
        Label("Inhabitants", demonym(c)),
        Label("Units", units(c)),
        Label("Government", or(c, &["government_type"], DASH)),
        Separator,
    ]);
    for (t, a) in [("Languages", languages(c)), ("Currencies", currencies(c))] {
        if !a.is_empty() {
            f.push(Tags(t, a))
        }
    }
    f.extend([
        Label(
            "Calling Code",
            joined(arr(c, &["calling_codes"]), ", ", DASH),
        ),
        Label(
            "Start of Week",
            nullish(c, &["date", "start_of_week"], DASH),
        ),
        Label(
            "Academic Year Start",
            day(get(c, &["date", "academic_year_start"])),
        ),
        Label(
            "Fiscal Year Start",
            day(get(c, &["date", "fiscal_year_start", "government"])),
        ),
        Label("Driving Side", nullish(c, &["cars", "driving_side"], DASH)),
        Label(
            "Vehicle Signs",
            joined(filtered(arr(c, &["cars", "signs"])), ", ", DASH),
        ),
        Label("Number Format", number_format(c)),
        Label("Top-Level Domain", joined_nonempty(arr(c, &["tlds"]))),
        Label("Timezones", joined_nonempty(arr(c, &["timezones"]))),
    ]);
    let b = arr(c, &["borders"]);
    if !b.is_empty() {
        f.push(Tags(
            "Borders",
            b.iter()
                .map(|v| names.get(&s(v)).map(|x| json!(x)).unwrap_or(v.clone()))
                .collect(),
        ))
    }
    let m = memberships(c);
    if !m.is_empty() {
        f.push(Separator);
        f.push(Tags("Memberships", m))
    }
    let links = [
        ("official", "Official Website"),
        ("wikipedia", "Wikipedia"),
        ("google_maps", "Google Maps"),
    ];
    if links.iter().any(|(k, _)| truth(&c["links"][k])) {
        f.push(Separator);
        for (k, t) in links {
            if truth(&c["links"][k]) {
                f.push(Link(t, s(&c["links"][k])))
            }
        }
    }
    f
}
pub fn text(c: &Value, names: &Names) -> String {
    let flag = or(c, &["flag", "emoji"], "");
    let mut lines = vec![
        format!(
            "{}{}",
            if flag.is_empty() {
                flag
            } else {
                format!("{flag} ")
            },
            common(c)
        ),
        String::new(),
    ];
    for f in fields(c, names) {
        match f {
            Field::Label(t, v) | Field::Link(t, v) => lines.push(format!("{t}: {v}")),
            Field::Tags(t, v) => lines.push(format!("{t}: {}", joined(v, ", ", ""))),
            _ => {}
        }
    }
    lines.join("\n")
}
pub fn names(countries: &[Value]) -> Names {
    let mut n = Names::new();
    for c in countries {
        if let Some(code) = get(c, &["codes", "alpha_3"])
            .and_then(Value::as_str)
            .map(js::trim)
            .filter(|s| !s.is_empty())
        {
            n.insert(code.into(), common(c));
        }
    }
    n
}
pub fn filter(countries: &[Value], hide: bool) -> Vec<Value> {
    countries
        .iter()
        .filter(|c| {
            !hide
                || get(c, &["codes", "alpha_3"])
                    .and_then(Value::as_str)
                    .is_some_and(|s| !js::trim(s).is_empty())
        })
        .cloned()
        .collect()
}
pub fn keywords(c: &Value) -> Vec<String> {
    let mut a = vec![c["names"]["official"].clone()];
    a.extend(arr(c, &["names", "alternates"]));
    a.extend([
        c["codes"]["alpha_2"].clone(),
        c["codes"]["alpha_3"].clone(),
        c["codes"]["ccn3"].clone(),
        c["region"].clone(),
        c["subregion"].clone(),
        json!(demonym(c)),
    ]);
    a.extend(arr(c, &["continents"]));
    a.extend(mapped(c, &["capitals"], "name"));
    a.extend(mapped(c, &["languages"], "name"));
    a.extend(mapped(c, &["currencies"], "name"));
    a.extend(arr(c, &["tlds"]));
    a.iter()
        .filter(|v| truth(v) && s(v) != DASH)
        .map(s)
        .collect()
}
pub fn matches(c: &Value, q: &str) -> bool {
    let q = js::trim(q).to_lowercase();
    q.is_empty()
        || std::iter::once(common(c))
            .chain(keywords(c))
            .any(|s| s.to_lowercase().contains(&q))
}
pub fn metadata(c: &Value, names: &Names) -> Value {
    let nodes=fields(c,names).into_iter().filter_map(|f|Some(match f{
        Field::Label(t,v)=>{let mut p=json!({"title":t,"text":v});if let Some(i)=match t{"Capital"=>Some("Icon.Building"),"Population"=>Some("Icon.TwoPeople"),"Calling Code"=>Some("Icon.Phone"),_=>None}{p["icon"]=json!(i)}json!({"type":"List.Item.Detail.Metadata.Label","props":p})},
        Field::Tags(t,a)=>json!({"type":"List.Item.Detail.Metadata.TagList","props":{"title":t},"children":a.iter().map(|v|json!({"type":"List.Item.Detail.Metadata.TagList.Item","props":{"text":js::s(v)}})).collect::<Vec<_>>()}),
        Field::Separator=>json!({"type":"List.Item.Detail.Metadata.Separator"}),
        Field::Link(t,v)=>{let target=crate::data::public_url(&v)?;json!({"type":"List.Item.Detail.Metadata.Link","props":{"title":t,"text":"Open","target":target}})}
    })).collect::<Vec<_>>();
    json!({"type":"List.Item.Detail.Metadata","children":nodes})
}
pub fn markdown(c: &Value) -> String {
    let mut a = vec![format!(
        "# {} {}",
        markdown_text(&nullish(c, &["flag", "emoji"], "")),
        markdown_text(&common(c))
    )];
    if let Some(u) = get(c, &["flag", "url_png"])
        .and_then(Value::as_str)
        .and_then(crate::data::flag_url)
    {
        a.push(format!("![flag]({u})"))
    }
    if truth(&c["flag"]["description"]) {
        a.push(format!(
            "_{}_",
            markdown_text(&s(&c["flag"]["description"]))
        ))
    }
    a.join("\n\n")
}
