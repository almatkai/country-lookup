use crate::{
    data::{self, Api},
    format, js,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tezbar_extension_sdk::{self as sdk, BoxFuture, CoreResult, Extension, ExtensionContext};
#[derive(Clone)]
enum Intent {
    Toggle,
    Refresh,
    Copy(String),
    Open(String),
    Retry(u64),
}
#[derive(Clone)]
struct Prepared {
    serial: u64,
    method: &'static str,
    params: Value,
    attempts: u8,
}
static OWNERSHIP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
struct Session {
    owner: u64,
    fetch_revision: u64,
    effect_revision: u64,
    effect_success: bool,
    epoch: u64,
    serial: u64,
    query: String,
    hide: bool,
    detail: bool,
    countries: Option<Arc<Vec<Value>>>,
    error: Option<String>,
    notice: Option<String>,
    actions: HashMap<String, Intent>,
    prepared: Option<Prepared>,
}
impl Session {
    fn new(hide: bool, query: String, detail: bool) -> Self {
        Self {
            owner: OWNERSHIP.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            fetch_revision: 0,
            effect_revision: 0,
            effect_success: false,
            epoch: 0,
            serial: 0,
            query,
            hide,
            detail,
            countries: None,
            error: None,
            notice: None,
            actions: HashMap::new(),
            prepared: None,
        }
    }
}
type Sessions = Arc<Mutex<HashMap<String, Arc<Mutex<Session>>>>>;
struct Registration {
    sessions: Sessions,
    id: String,
    slot: Arc<Mutex<Session>>,
}
impl Drop for Registration {
    fn drop(&mut self) {
        let mut m = self.sessions.lock().unwrap();
        if m.get(&self.id).is_some_and(|s| Arc::ptr_eq(s, &self.slot)) {
            m.remove(&self.id);
        }
    }
}
struct Cache {
    fingerprint: String,
    time: Instant,
    countries: Arc<Vec<Value>>,
}
pub struct Adapter {
    api: Api,
    sessions: Sessions,
    cache: tokio::sync::Mutex<Option<Cache>>,
}
impl Default for Adapter {
    fn default() -> Self {
        Self {
            api: Api::default(),
            sessions: Arc::default(),
            cache: tokio::sync::Mutex::new(None),
        }
    }
}
/// An unfinished operation can have performed irreversible work. Its drop records
/// uncertainty independently of the SDK's transactional Value, never replays it.
struct Attempt {
    slot: Arc<Mutex<Session>>,
    fetch: bool,
    finished: bool,
}
impl Drop for Attempt {
    fn drop(&mut self) {
        if !self.finished {
            let mut s = self.slot.lock().unwrap();
            s.epoch += 1;
            s.actions.clear();
            if self.fetch {
                s.countries = None;
                s.error = Some(
                    "Country loading was interrupted. Choose Retry to make another request.".into(),
                )
            } else {
                s.notice = Some(
                    "The action was interrupted; its result is uncertain. Retry only if needed."
                        .into(),
                )
            }
        }
    }
}
fn config(p: &Value) -> Result<(&str, bool), String> {
    let prefs = &p["preferences"];
    let key=prefs["apiKey"].as_str().map(js::trim).filter(|s|!s.is_empty()).ok_or("Set API Key in Country Lookup preferences. Get a key at https://restcountries.com/sign-up, then reopen Search Countries.")?;
    if key.len() > 4096 || !key.is_ascii() || key.bytes().any(|b| b.is_ascii_control()) {
        return Err(
            "API Key must be a valid single-line credential. Update Country Lookup preferences."
                .into(),
        );
    }
    let hide = match prefs["entriesWithoutIsoCode"].as_str() {
        Some("show") => false,
        Some("hide") => true,
        _ => return Err(
            "Choose Show Countries in Country Lookup preferences, then reopen Search Countries."
                .into(),
        ),
    };
    Ok((key, hide))
}
fn detail_value(v: &Value) -> Result<bool, String> {
    if let Some(v) = v.get("showing-detail") {
        v.as_bool().ok_or("Country Lookup showing-detail cache must be a boolean. Repair its private storage.").map_err(str::to_owned)
    } else {
        Ok(true)
    }
}
async fn read_detail(ctx: &ExtensionContext) -> Result<bool, String> {
    let v = crate::store::transaction(ctx, "country-lookup", 65536, |v| {
        detail_value(v)?;
        Ok(false)
    })
    .await?;
    detail_value(&v)
}
async fn toggle_detail(ctx: &ExtensionContext) -> Result<bool, String> {
    let v = crate::store::transaction(ctx, "country-lookup", 65536, |v| {
        v["showing-detail"] = json!(!detail_value(v)?);
        Ok(true)
    })
    .await?;
    detail_value(&v)
}
pub fn ack(v: Value) -> Result<(), String> {
    let m = v
        .as_object()
        .ok_or("The host did not acknowledge the action")?;
    if m.get("ok") != Some(&Value::Bool(true))
        || m.keys().any(|k| k != "ok" && k != "message")
        || m.get("message").is_some_and(|v| !v.is_string())
    {
        return Err("The host did not acknowledge the action".into());
    }
    Ok(())
}
impl Adapter {
    async fn fetch(
        &self,
        ctx: &ExtensionContext,
        slot: &Arc<Mutex<Session>>,
        key: &str,
        force: bool,
    ) -> Result<(), String> {
        {
            let mut s = slot.lock().unwrap();
            s.epoch += 1;
            s.fetch_revision += 1;
            s.countries = None;
            s.error = Some(
                "Country loading was interrupted. Choose Retry to make another request.".into(),
            );
            s.notice = None;
            s.prepared = None;
            s.actions.clear();
        }
        let mut attempt = Attempt {
            slot: slot.clone(),
            fetch: true,
            finished: false,
        };
        let fingerprint = data::fingerprint(key, &ctx.paths.data_dir.to_string_lossy());
        let mut cache = self.cache.lock().await;
        let result = async {
            if force {
                *cache = None;
                crate::store::transaction(ctx, "country-lookup-cache", 32 * 1024 * 1024, |v| {
                    v.as_object_mut().unwrap().shift_remove("entry");
                    Ok(true)
                })
                .await?;
            }
            if let Some(c) = cache.as_ref().filter(|c| {
                c.fingerprint == fingerprint && c.time.elapsed() <= Duration::from_secs(4 * 60 * 60)
            }) {
                return Ok(c.countries.clone());
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "System clock is invalid")?
                .as_secs();
            let stored =
                crate::store::transaction(ctx, "country-lookup-cache", 32 * 1024 * 1024, |_| {
                    Ok(false)
                })
                .await?;
            let entry = &stored["entry"];
            if stored.get("entry").is_some()
                && (!entry.is_object()
                    || entry["fingerprint"]
                        .as_str()
                        .is_none_or(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
                    || entry["time"].as_u64().is_none()
                    || entry["countries"]
                        .as_array()
                        .is_none_or(|a| a.len() > data::MAX_TOTAL))
            {
                return Err("Country cache is malformed. Choose Refresh to replace it.".into());
            }

            if entry["fingerprint"] == fingerprint
                && entry["time"]
                    .as_u64()
                    .is_some_and(|t| t <= now && now - t <= 4 * 60 * 60)
            {
                let c = entry["countries"]
                    .as_array()
                    .filter(|c| c.len() <= data::MAX_TOTAL)
                    .ok_or("Country cache is malformed. Choose Refresh to replace it.")?;
                if data::contains(&entry["countries"], key) {
                    return Err(
                        "Country cache contained private authentication data and was rejected"
                            .into(),
                    );
                }
                for country in c {
                    data::validate_country(country)?;
                }
                let countries = Arc::new(c.clone());
                *cache = Some(Cache {
                    fingerprint: fingerprint.clone(),
                    time: Instant::now()
                        - Duration::from_secs(now - entry["time"].as_u64().unwrap()),
                    countries: countries.clone(),
                });
                return Ok(countries);
            }
            let countries = Arc::new(self.api.all(key).await?);
            let record = json!({"fingerprint":fingerprint,"time":now,"countries":*countries});
            crate::store::transaction(ctx, "country-lookup-cache", 32 * 1024 * 1024, |v| {
                v["entry"] = record;
                Ok(true)
            })
            .await?;
            *cache = Some(Cache {
                fingerprint,
                time: Instant::now(),
                countries: countries.clone(),
            });
            Ok::<_, String>(countries)
        }
        .await;
        // No await between authoritative result and SDK commit; cancellation drops unfinished attempts.
        let mut s = slot.lock().unwrap();
        s.epoch += 1;
        match result {
            Ok(c) => {
                s.countries = Some(c);
                s.error = None
            }
            Err(e) => {
                s.countries = None;
                s.error = Some(e)
            }
        }
        attempt.finished = true;
        Ok(())
    }
    fn render(&self, slot: &Arc<Mutex<Session>>) -> Value {
        let mut s = slot.lock().unwrap();
        s.epoch += 1;
        let epoch = format!("{}:{}", s.owner, s.epoch);
        s.actions.clear();
        let mut actions = vec![];
        let countries = s
            .countries
            .as_ref()
            .map(|c| format::filter(c, s.hide))
            .unwrap_or_default();
        let names = format::names(&countries);
        let detail = s.detail;
        let query = s.query.clone();
        let mut children = Vec::new();
        for (i, c) in countries
            .iter()
            .enumerate()
            .filter(|(_, c)| format::matches(c, &query))
        {
            let mut ids = vec![];
            let mut add = |kind: &str, title: &str, intent: Intent| {
                let id = format!("{epoch}:{i}:{kind}");
                s.actions.insert(id.clone(), intent);
                actions.push(json!({"id":id,"title":title,"kind":"action"}));
                ids.push(id);
            };
            add("toggle", "Toggle Details", Intent::Toggle);
            for (k, t) in [
                ("google_maps", "Open in Google Maps"),
                ("open_street_maps", "Open in OpenStreetMap"),
                ("wikipedia", "Open Wikipedia"),
            ] {
                if let Some(u) = c["links"][k].as_str().and_then(data::public_url) {
                    add(k, t, Intent::Open(u));
                }
            }
            add(
                "text",
                "Copy as Text",
                Intent::Copy(format::text(c, &names)),
            );
            add("name", "Copy Country Name", Intent::Copy(format::common(c)));
            if js::truth(&c["codes"]["alpha_2"]) {
                add(
                    "code",
                    "Copy Country Code",
                    Intent::Copy(js::s(&c["codes"]["alpha_2"])),
                )
            }
            if js::truth(&c["flag"]["emoji"]) {
                add(
                    "emoji",
                    "Copy Flag Emoji",
                    Intent::Copy(js::s(&c["flag"]["emoji"])),
                )
            }
            // Raw JSON is generated from the original record only, with JavaScript key/number rules.
            add("raw", "Copy Raw JSON", Intent::Copy(js::stringify(c)));
            add("refresh", "Refresh", Intent::Refresh);
            let icon = c["flag"]["url_png"]
                .as_str()
                .and_then(data::flag_url)
                .map(|u| json!({"source":u}))
                .unwrap_or(json!("Icon.Globe"));
            let mut props = json!({"id":format!("country:{i}"),"title":format::common(c),"icon":icon,"actionIds":ids,"keywords":format::keywords(c)});
            if detail {
                props["detail"] = json!({"type":"List.Item.Detail","props":{"markdown":format::markdown(c),"metadata":format::metadata(c,&names)}})
            } else {
                props["subtitle"] = json!(js::nullish(c, &["names", "official"], ""));
                props["accessories"] = json!([{"text":c["region"],"icon":"Icon.Map"},{"tag":js::number(c.get("population"))}]);
            }
            children.push(json!({"type":"List.Item","props":props}));
        }
        if let Some(p) = s
            .prepared
            .clone()
            .filter(|p| p.attempts < 3 && !s.effect_success)
        {
            let id = format!("{epoch}:retry:{}", p.serial);
            s.actions.insert(id.clone(), Intent::Retry(p.serial));
            for child in &mut children {
                if let Some(ids) = child["props"]["actionIds"].as_array_mut() {
                    ids.push(json!(id));
                }
            }

            actions.insert(
                0,
                json!({"id":id,"title":"Retry Previous Action (May Repeat)","kind":"action"}),
            );
        }
        let refresh = format!("{epoch}:refresh");
        s.actions.insert(refresh.clone(), Intent::Refresh);
        actions.push(json!({"id":refresh.clone(),"title":if s.error.is_some(){"Retry"}else{"Refresh"},"kind":"action"}));
        if children.is_empty() {
            children.push(json!({"type":"List.EmptyView","props":{"title":if s.error.is_some(){"Could not load countries"}else{"No countries found"},"description":s.error.clone().unwrap_or_else(||"Try a different search term.".into()),"actionIds":[refresh]}}))
        }
        let mut root = json!({"type":"List","props":{"navigationTitle":"Country Lookup","searchBarPlaceholder":"Search countries by name, capital, language…","searchText":s.query,"__hasServerSearch":true,"isShowingDetail":detail},"children":children});
        if let Some(error) = &s.error {
            root["props"]["navigationTitle"] = json!(format!(
                "Country Lookup — Could not load countries. {error}"
            ));
        } else if let Some(n) = &s.notice {
            root["props"]["navigationTitle"] = json!(format!("Country Lookup — {n}"));
        }
        let mut v = sdk::view("Country Lookup", root, actions);
        v["refreshMs"] = Value::Null;
        if serde_json::to_vec(&v).map_or(true, |bytes| bytes.len() > 8 * 1024 * 1024) {
            s.actions
                .retain(|_, intent| matches!(intent, Intent::Refresh));
            let mut fallback = sdk::view(
                "Country Lookup",
                json!({"type":"List","props":{"navigationTitle":"Country Lookup — Too much detail to display. Search for fewer countries.","searchBarPlaceholder":"Search for fewer countries…","searchText":s.query,"__hasServerSearch":true},"children":[{"type":"List.EmptyView","props":{"title":"Too much detail to display","description":"Search for fewer countries. The complete country set is already loaded."}}]}),
                vec![],
            );
            fallback["refreshMs"] = Value::Null;
            return fallback;
        }
        v
    }
    async fn effect(
        &self,
        ctx: &ExtensionContext,
        slot: &Arc<Mutex<Session>>,
        intent: Intent,
    ) -> CoreResult {
        let p = {
            let mut s = slot.lock().unwrap();
            s.epoch += 1;
            s.actions.clear();
            let p = match intent {
                Intent::Copy(text) => {
                    if text.len() > 1024 * 1024 {
                        return Err("Copy content exceeds the 1 MiB limit".into());
                    }

                    s.serial += 1;
                    Prepared {
                        serial: s.serial,
                        method: "host.clipboard.write",
                        params: json!({"text":text}),
                        attempts: 1,
                    }
                }
                Intent::Open(target) => {
                    s.serial += 1;
                    Prepared {
                        serial: s.serial,
                        method: "host.shell.open",
                        params: json!({"target":target}),
                        attempts: 1,
                    }
                }
                Intent::Retry(serial) => {
                    let mut p = s
                        .prepared
                        .clone()
                        .filter(|p| p.serial == serial && p.attempts < 3)
                        .ok_or("Previous action is no longer retryable")?;
                    p.attempts += 1;
                    p
                }
                _ => return Err("Invalid effect".into()),
            };
            s.effect_revision += 1;
            s.effect_success = false;
            s.prepared = Some(p.clone());
            s.notice = Some("The action result is uncertain. Retry only if needed.".into());
            p
        };
        let mut attempt = Attempt {
            slot: slot.clone(),
            fetch: false,
            finished: false,
        };
        let result =
            tokio::time::timeout(Duration::from_secs(10), ctx.host(p.method, p.params)).await;
        let ok = matches!(result,Ok(Ok(v)) if ack(v.clone()).is_ok());
        let mut s = slot.lock().unwrap();
        if s.prepared.as_ref().is_some_and(|q| q.serial == p.serial) {
            if ok {
                s.effect_success = true;
                s.notice = Some(
                    if p.method == "host.clipboard.write" {
                        "Copied"
                    } else {
                        "Opened"
                    }
                    .into(),
                )
            } else {
                s.notice=Some("The host did not confirm the action. Its result may be uncertain; retry only if needed.".into())
            }
        }
        attempt.finished = true;
        drop(s);
        Ok(self.render(slot))
    }
    async fn handle_inner(
        &self,
        ctx: &ExtensionContext,
        op: &str,
        p: &Value,
        state: &mut Value,
    ) -> CoreResult {
        let id = p["sessionId"]
            .as_str()
            .filter(|s| s.len() <= 256)
            .ok_or("Invalid Country Lookup session")?
            .to_owned();
        if op == "extension:dispose-session" {
            self.sessions.lock().unwrap().remove(&id);
            return Ok(sdk::unchanged());
        }
        let (key, hide) = config(p)?;
        if op == "extension:run-command" {
            let detail = read_detail(ctx).await?;
            let query = p["searchText"].as_str().unwrap_or("");
            if query.len() > 1024 {
                return Err("Search is limited to 1,024 bytes".into());
            }
            let slot = Arc::new(Mutex::new(Session::new(hide, query.into(), detail)));
            {
                let mut m = self.sessions.lock().unwrap();
                if m.len() >= 128 || m.contains_key(&id) {
                    return Err("Country Lookup session limit reached".into());
                }
                m.insert(id.clone(), slot.clone());
            }
            let registration = Registration {
                sessions: self.sessions.clone(),
                id: id.clone(),
                slot: slot.clone(),
            };
            ctx.spawn(async move {
                let _registration = registration;
                std::future::pending::<()>().await
            })?;
            self.fetch(ctx, &slot, key, false).await?;
            *state = json!({"session":id,"query":query,"fetchRevision":slot.lock().unwrap().fetch_revision,"effectRevision":0});
            return Ok(self.render(&slot));
        }
        if state["session"] != id {
            return Err("Country Lookup session is not active; reopen Search Countries".into());
        }
        let slot = self
            .sessions
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or("Country Lookup session expired")?;
        // Revalidate private state on every event; storage writes cannot be rolled back by cancellation.
        let detail = read_detail(ctx).await?;
        {
            let mut s = slot.lock().unwrap();
            if state["fetchRevision"].as_u64() != Some(s.fetch_revision) {
                state["fetchRevision"] = json!(s.fetch_revision);
                s.countries = None;
                s.error = Some(
                    "Country loading was interrupted. Choose Retry to make another request.".into(),
                );
                s.actions.clear();
            }
            if s.effect_success {
                if state["effectRevision"].as_u64() == Some(s.effect_revision) {
                    s.prepared = None;
                } else {
                    s.effect_success = false;
                    s.notice=Some("The action response was discarded; its result is uncertain. Retry only if needed.".into());
                }
            }
            s.detail = detail;
            s.hide = hide;
            s.query = state["query"].as_str().unwrap_or("").to_owned();
        }
        match op {
            "extension:search-text-changed" => {
                let q = p["searchText"].as_str().ok_or("Missing search text")?;
                if q.len() > 1024 {
                    return Err("Search is limited to 1,024 bytes".into());
                }
                slot.lock().unwrap().query = q.into();
                state["query"] = json!(q);
            }
            "extension:refresh-session" | "extension:load-more" => {}
            "extension:invoke-action" => {
                let a = p["actionId"].as_str().ok_or("Missing action ID")?;
                let intent = {
                    let mut s = slot.lock().unwrap();
                    let i = s
                        .actions
                        .remove(a)
                        .ok_or("This action is stale; use the current country list")?;
                    s.epoch += 1;
                    s.actions.clear();
                    i
                };
                match intent {
                    Intent::Toggle => {
                        let next = toggle_detail(ctx).await?;
                        slot.lock().unwrap().detail = next
                    }
                    Intent::Refresh => {
                        self.fetch(ctx, &slot, key, true).await?;
                        state["fetchRevision"] = json!(slot.lock().unwrap().fetch_revision);
                    }
                    i => {
                        let result = self.effect(ctx, &slot, i).await?;
                        state["effectRevision"] = json!(slot.lock().unwrap().effect_revision);
                        return Ok(result);
                    }
                }
            }
            _ => return Err("Unsupported Country Lookup operation".into()),
        }
        Ok(self.render(&slot))
    }
}
impl Extension for Adapter {
    fn id(&self) -> &'static str {
        "com.tezbar.native.country-lookup"
    }
    fn version(&self) -> &'static str {
        "0.2.0"
    }
    fn commands(&self) -> &'static [&'static str] {
        &["index"]
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        op: &'a str,
        p: &'a Value,
        state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            if command != "index" {
                return Err("Unknown Country Lookup command".into());
            }
            self.handle_inner(ctx, op, p, state).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tezbar_extension_sdk::{ExtensionPaths, TestContext};
    fn temp() -> tempfile::TempDir {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");
        std::fs::create_dir_all(&p).unwrap();
        tempfile::tempdir_in(p).unwrap()
    }
    fn ctx(dir: &std::path::Path) -> TestContext {
        TestContext::new(
            ExtensionPaths {
                package_dir: dir.into(),
                data_dir: dir.into(),
                temp_dir: dir.into(),
            },
            &["clipboard.write", "shell.open"],
        )
    }
    fn payload(id: &str) -> Value {
        json!({"sessionId":id,"preferences":{"apiKey":"PUBLIC_SYNTHETIC_COUNTRY_TEST","entriesWithoutIsoCode":"show"}})
    }
    fn seed(a: &Adapter, c: &TestContext) {
        let countries = js::parse(include_str!("../qa/oracle.json").as_bytes()).unwrap()["cases"]
            .as_array()
            .unwrap()
            .iter()
            .take(2)
            .map(|v| js::parse(v["input"].as_str().unwrap().as_bytes()).unwrap())
            .collect::<Vec<_>>();
        *a.cache.try_lock().unwrap() = Some(Cache {
            fingerprint: data::fingerprint(
                "PUBLIC_SYNTHETIC_COUNTRY_TEST",
                &c.context.paths.data_dir.to_string_lossy(),
            ),
            time: Instant::now(),
            countries: Arc::new(countries),
        });
    }
    fn action(v: &Value, title: &str) -> String {
        v["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["title"] == title)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .into()
    }
    #[test]
    fn strict_acknowledgements() {
        for v in [
            Value::Null,
            json!(true),
            json!(false),
            json!({"ok":false}),
            json!({"ok":true,"error":"bad"}),
            json!({"ok":true,"success":false}),
            json!({"ok":true,"message":null}),
        ] {
            assert!(ack(v).is_err())
        }
        assert!(ack(json!({"ok":true})).is_ok());
        assert!(ack(json!({"ok":true,"message":"done"})).is_ok());
    }
    #[tokio::test]
    async fn real_adapter_actions_raw_toggle_search_stale_and_prepared_retry() {
        let dir = temp();
        let c = ctx(dir.path());
        let a = Adapter::default();
        seed(&a, &c);
        let mut state = json!({});
        let mut p = payload("test");
        let v = a
            .handle(&c.context, "index", "extension:run-command", &p, &mut state)
            .await
            .unwrap();
        assert!(v["root"]["children"][0]["props"].get("detail").is_some());
        assert!(!v.to_string().contains("PUBLIC_SYNTHETIC_COUNTRY_TEST"));
        assert!(!state.to_string().contains("apiKey"));
        let old = action(&v, "Copy Raw JSON");
        p["actionId"] = json!(old);
        c.reply(Ok(json!({"ok":true,"error":"contradiction"})));
        let failed = c
            .run(a.handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state,
            ))
            .await
            .unwrap();
        assert_eq!(c.calls().len(), 1);
        let exact = c.calls()[0]["params"].clone();
        let retry = action(&failed, "Retry Previous Action (May Repeat)");
        p["actionId"] = json!(old);
        assert!(a
            .handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state
            )
            .await
            .is_err());
        assert_eq!(c.calls().len(), 1);
        let unchanged = a
            .handle(
                &c.context,
                "index",
                "extension:refresh-session",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(c.calls().len(), 1);
        let current = action(&unchanged, "Retry Previous Action (May Repeat)");
        assert_ne!(retry, current);
        p["actionId"] = json!(current);
        c.reply(Ok(json!({"ok":true})));
        let v = c
            .run(a.handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state,
            ))
            .await
            .unwrap();
        assert_eq!(c.calls()[1]["params"], exact);
        assert!(v["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["title"] != "Retry Previous Action (May Repeat)"));
        p["actionId"] = json!(action(&v, "Toggle Details"));
        let v = a
            .handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert!(v["root"]["children"][0]["props"].get("detail").is_none());
        p["searchText"] = json!("example language");
        let v = a
            .handle(
                &c.context,
                "index",
                "extension:search-text-changed",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(v["root"]["children"].as_array().unwrap().len(), 1);
        assert_eq!(c.calls().len(), 2);
        let path = dir.path().join("country-lookup/storage.json");
        let mut stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        stored["unknown"] = json!({"preserve":true});
        std::fs::write(&path, stored.to_string()).unwrap();
        p["actionId"] = json!(action(&v, "Toggle Details"));
        a.handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &p,
            &mut state,
        )
        .await
        .unwrap();
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored["unknown"]["preserve"], true);
        assert_eq!(stored["showing-detail"], true);
        std::fs::write(path, b"{broken").unwrap();
        assert!(a
            .handle(
                &c.context,
                "index",
                "extension:refresh-session",
                &p,
                &mut state
            )
            .await
            .is_err());
    }
    #[tokio::test]
    async fn missing_key_no_traffic_and_drop_guard_cleanup() {
        let dir = temp();
        let c = ctx(dir.path());
        let a = Adapter::default();
        let mut p = payload("bad");
        p["preferences"]["apiKey"] = json!("  ");
        let mut s = json!({});
        assert!(a
            .handle(&c.context, "index", "extension:run-command", &p, &mut s)
            .await
            .is_err());
        assert!(a.sessions.lock().unwrap().is_empty());
        assert!(c.calls().is_empty());
        let slot = Arc::new(Mutex::new(Session::new(false, String::new(), true)));
        a.sessions
            .lock()
            .unwrap()
            .insert("guard".into(), slot.clone());
        let g = Registration {
            sessions: a.sessions.clone(),
            id: "guard".into(),
            slot,
        };
        drop(g);
        assert!(a.sessions.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn concurrent_toggle_transactions_are_lossless_and_bounded() {
        let dir = temp();
        let c = ctx(dir.path());
        let (a, b) = tokio::join!(toggle_detail(&c.context), toggle_detail(&c.context));
        assert_ne!(a.unwrap(), b.unwrap());
        assert!(read_detail(&c.context).await.unwrap());
        let p = dir.path().join("country-lookup/storage.json");
        std::fs::write(&p, json!({"showing-detail":"false"}).to_string()).unwrap();
        assert!(read_detail(&c.context).await.is_err());
        std::fs::write(&p, vec![b' '; 65537]).unwrap();
        assert!(toggle_detail(&c.context).await.is_err());
    }
    #[tokio::test]
    async fn discarded_success_and_search_reconcile_from_sdk_committed_state() {
        let dir = temp();
        let c = ctx(dir.path());
        let a = Adapter::default();
        seed(&a, &c);
        let mut p = payload("discarded");
        let mut state = json!({});
        let view = a
            .handle(&c.context, "index", "extension:run-command", &p, &mut state)
            .await
            .unwrap();
        // Model a handler completing just before SDK cancellation rejects its staged Value.
        let mut discarded = state.clone();
        p["actionId"] = json!(action(&view, "Copy Country Name"));
        c.reply(Ok(json!({"ok":true})));
        c.run(a.handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &p,
            &mut discarded,
        ))
        .await
        .unwrap();
        let uncertain = a
            .handle(
                &c.context,
                "index",
                "extension:refresh-session",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        let retry = action(&uncertain, "Retry Previous Action (May Repeat)");
        assert!(uncertain["root"]["children"][0]["props"]["actionIds"]
            .as_array()
            .unwrap()
            .contains(&json!(retry)));
        p["actionId"] = json!(retry);
        c.reply(Ok(json!({"ok":true})));
        c.run(a.handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &p,
            &mut state,
        ))
        .await
        .unwrap();
        assert_eq!(c.calls()[0]["params"], c.calls()[1]["params"]);
        let mut discarded = state.clone();
        p["searchText"] = json!("discarded search");
        a.handle(
            &c.context,
            "index",
            "extension:search-text-changed",
            &p,
            &mut discarded,
        )
        .await
        .unwrap();
        let current = a
            .handle(
                &c.context,
                "index",
                "extension:refresh-session",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(current["root"]["props"]["searchText"], "");
        // A discarded fetch cannot publish its data on heartbeat; reconciling uncertainty
        // commits a revision so a subsequent deliberate Retry remains usable.
        let slot = a.sessions.lock().unwrap()["discarded"].clone();
        {
            let mut s = slot.lock().unwrap();
            s.fetch_revision += 1;
            s.countries = Some(Arc::new(vec![
                json!({"names":{"common":"discarded fetch data"}}),
            ]));
        }
        let failure = a
            .handle(
                &c.context,
                "index",
                "extension:refresh-session",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(
            failure["root"]["children"][0]["props"]["title"],
            "Could not load countries"
        );
        p["actionId"] = json!(action(&failure, "Retry"));
        let retried = a
            .handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(
            retried["root"]["children"][0]["props"]["title"],
            "Could not load countries"
        );
        assert_eq!(
            state["fetchRevision"].as_u64(),
            Some(slot.lock().unwrap().fetch_revision)
        );
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    #[tokio::test]
    async fn cancelled_open_during_http_has_no_authoritative_registration() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/countries/v5", listener.local_addr().unwrap());
        let adapter = Adapter {
            api: Api::local(endpoint),
            ..Adapter::default()
        };
        let sessions = adapter.sessions.clone();
        let (accepted, mut reached) = tokio::sync::mpsc::channel(3);
        let producer = tokio::spawn(async move {
            let mut sockets = vec![];
            for _ in 0..3 {
                let (s, _) = listener.accept().await.unwrap();
                sockets.push(s);
                accepted.send(()).await.unwrap();
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        });
        let (client, server) = tokio::io::duplex(256 * 1024);
        let (input, output) = tokio::io::split(server);
        let runner = tokio::spawn(sdk::serve_io(adapter, input, output));
        let (r, mut w) = tokio::io::split(client);
        let mut r = BufReader::new(r);
        let mut line = String::new();
        r.read_line(&mut line).await.unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");
        std::fs::create_dir_all(&root).unwrap();
        let temp = tempfile::tempdir_in(root).unwrap();
        let path = temp.path().to_str().unwrap();
        let initialize = json!({"type":"request","id":"h:00000000-0000-4000-8000-000000000001","method":"initialize","params":{"protocol":1,"extensionId":"com.tezbar.native.country-lookup","platform":"macOS","packageDir":path,"dataDir":path,"tempDir":path,"grantedCapabilities":[]}});
        w.write_all(format!("{initialize}\n").as_bytes())
            .await
            .unwrap();
        line.clear();
        r.read_line(&mut line).await.unwrap();
        let open = json!({"type":"request","id":"h:00000000-0000-4000-8000-000000000002","method":"session.open","params":{"sessionId":"cancel-during-fetch","commandName":"index","preferences":{"apiKey":"PUBLIC_SYNTHETIC_COUNTRY_TEST","entriesWithoutIsoCode":"show"},"argumentValues":{}}});
        w.write_all(format!("{open}\n").as_bytes()).await.unwrap();
        for _ in 0..3 {
            tokio::time::timeout(Duration::from_secs(1), reached.recv())
                .await
                .unwrap();
        }
        assert_eq!(sessions.lock().unwrap().len(), 1);
        w.write_all(b"{\"type\":\"cancel\",\"requestId\":\"h:00000000-0000-4000-8000-000000000002\",\"reason\":\"qa\"}\n").await.unwrap();
        line.clear();
        r.read_line(&mut line).await.unwrap();
        assert!(serde_json::from_str::<Value>(&line)
            .unwrap()
            .get("error")
            .is_some());
        for _ in 0..100 {
            if sessions.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await
        }
        assert!(sessions.lock().unwrap().is_empty());
        w.shutdown().await.unwrap();
        assert!(runner.await.unwrap().is_ok());
        producer.await.unwrap();
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn failed_refresh_drops_data_and_requires_deliberate_retry() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");
        std::fs::create_dir_all(&root).unwrap();
        let temp = tempfile::tempdir_in(root).unwrap();
        let c = sdk::TestContext::new(
            sdk::ExtensionPaths {
                data_dir: temp.path().into(),
                package_dir: temp.path().into(),
                temp_dir: temp.path().into(),
            },
            &[],
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/countries/v5", l.local_addr().unwrap());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = count.clone();
        let server = tokio::spawn(async move {
            for _ in 0..6 {
                let (mut s, _) = l.accept().await.unwrap();
                let mut b = [0; 8192];
                assert!(s.read(&mut b).await.unwrap() > 0);
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                s.write_all(
                    b"HTTP/1.1 500 Synthetic\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            }
        });
        let a = Adapter {
            api: Api::local(endpoint),
            ..Adapter::default()
        };
        let key = "PUBLIC_SYNTHETIC_COUNTRY_TEST";
        *a.cache.lock().await = Some(Cache {
            fingerprint: data::fingerprint(key, &temp.path().to_string_lossy()),
            time: Instant::now(),
            countries: Arc::new(vec![json!({"names":{"common":"Previous data"}})]),
        });
        let mut p = json!({"sessionId":"refresh","preferences":{"apiKey":key,"entriesWithoutIsoCode":"show"}});
        let mut state = json!({});
        let view = a
            .handle(&c.context, "index", "extension:run-command", &p, &mut state)
            .await
            .unwrap();
        assert_eq!(
            view["root"]["children"][0]["props"]["title"],
            "Previous data"
        );
        p["actionId"] = view["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["title"] == "Refresh")
            .unwrap()["id"]
            .clone();
        let failed = a
            .handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(
            failed["root"]["children"][0]["props"]["title"],
            "Could not load countries"
        );
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 3);
        p["searchText"] = json!("a");
        let still = a
            .handle(
                &c.context,
                "index",
                "extension:search-text-changed",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 3);
        let still = a
            .handle(
                &c.context,
                "index",
                "extension:refresh-session",
                &p,
                &mut state,
            )
            .await
            .unwrap_or(still);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 3);
        p["actionId"] = still["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["title"] == "Retry")
            .unwrap()["id"]
            .clone();
        let failed = a
            .handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &p,
                &mut state,
            )
            .await
            .unwrap();
        assert_eq!(
            failed["root"]["children"][0]["props"]["title"],
            "Could not load countries"
        );
        server.await.unwrap();
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 6);
    }
}

#[cfg(test)]
mod action_oracle_tests {
    use super::*;
    #[test]
    fn actual_source_action_order_and_exact_copy_open_intents() {
        let oracle: Value = serde_json::from_str(include_str!("../qa/oracle.json")).unwrap();
        let adapter = Adapter::default();
        for case in oracle["cases"].as_array().unwrap() {
            let country = js::parse(case["input"].as_str().unwrap().as_bytes()).unwrap();
            let mut session = Session::new(false, String::new(), true);
            session.countries = Some(Arc::new(vec![country]));
            let slot = Arc::new(Mutex::new(session));
            let v = adapter.render(&slot);
            let ids = v["root"]["children"][0]["props"]["actionIds"]
                .as_array()
                .unwrap();
            let row = v["actions"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|a| ids.contains(&a["id"]))
                .collect::<Vec<_>>();
            let source = case["actions"].as_array().unwrap();
            assert_eq!(
                row.iter().map(|a| a["title"].clone()).collect::<Vec<_>>(),
                source
                    .iter()
                    .map(|a| a["title"].clone())
                    .collect::<Vec<_>>()
            );
            let s = slot.lock().unwrap();
            for (a, b) in row.iter().zip(source) {
                let intent = s.actions.get(a["id"].as_str().unwrap()).unwrap();
                if a["title"] == "Copy as Text" {
                    continue;
                }
                match intent {
                    Intent::Copy(text) => assert_eq!(text, b["content"].as_str().unwrap()),
                    Intent::Open(url) => assert_eq!(url, b["url"].as_str().unwrap()),
                    _ => {}
                }
            }
        }
    }
}
