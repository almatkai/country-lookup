use crate::js;
use reqwest::{
    header::{HeaderValue, AUTHORIZATION},
    redirect::Policy,
    Client,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, time::Duration};
pub const ENDPOINT: &str = "https://api.restcountries.com/countries/v5";
pub const MAX_TOTAL: usize = 2000;
pub const MAX_BYTES: usize = 24 * 1024 * 1024;
#[derive(Clone)]
pub struct Api {
    client: Client,
    endpoint: String,
}
impl Default for Api {
    fn default() -> Self {
        Self::new(ENDPOINT.into()).expect("Fixed TLS client configuration")
    }
}
impl Api {
    fn new(endpoint: String) -> Result<Self, String> {
        Ok(Self {
            client: Client::builder()
                .redirect(Policy::none())
                .https_only(true)
                .no_proxy()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|_| "Could not initialize secure country connection")?,
            endpoint,
        })
    }
    #[cfg(test)]
    pub(crate) fn local(endpoint: String) -> Self {
        Self {
            client: Client::builder()
                .no_proxy()
                .redirect(Policy::none())
                .connect_timeout(Duration::from_millis(200))
                .timeout(Duration::from_millis(500))
                .build()
                .unwrap(),
            endpoint,
        }
    }
    async fn page(&self, key: &str, offset: usize) -> Result<Page, String> {
        if self.endpoint == ENDPOINT
            && (key.starts_with("PUBLIC_SYNTHETIC_")
                || ["test", "fake", "dummy", "YOUR_API_KEY"].contains(&key))
        {
            return Err("Set a valid API Key in Country Lookup preferences; placeholder credentials are not sent.".into());
        }
        let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
            "API Key must be a valid single-line credential. Update Country Lookup preferences."
        })?;
        auth.set_sensitive(true);
        let mut r = self
            .client
            .get(&self.endpoint)
            .query(&[("limit", 100), ("offset", offset)])
            .header(AUTHORIZATION, auth)
            .send()
            .await
            .map_err(|_| {
                "Could not connect to REST Countries securely. Check the connection, then Retry."
                    .to_string()
            })?;
        if !r.status().is_success() {
            let msg = match r.status().as_u16() {
                401 => "API Key was rejected. Update Country Lookup preferences.",
                403 => "REST Countries denied access or the account quota is exhausted.",
                429 => "REST Countries rate limit reached. Wait before Retry.",
                300..=399 => "REST Countries redirected the request; redirects are disabled.",
                _ => "REST Countries request failed. Retry later.",
            };
            return Err(msg.into());
        }
        if r.content_length().is_some_and(|n| n > 8 * 1024 * 1024) {
            return Err("Country response exceeds the 8 MiB page limit".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = r
            .chunk()
            .await
            .map_err(|_| "Could not read the country response".to_string())?
        {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return Err("Country response exceeds the 8 MiB page limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        parse_page(&bytes, key, offset)
    }
    pub async fn all(&self, key: &str) -> Result<Vec<Value>, String> {
        tokio::time::timeout(Duration::from_secs(45), async {
            // Exact source's first three speculative offsets, each polled in this owned future.
            let (a, b, c) =
                tokio::join!(self.page(key, 0), self.page(key, 100), self.page(key, 200));
            let first = a?;
            let total = first.total;
            if total > MAX_TOTAL {
                return Err("Country count exceeds the 2,000 record / 20 page limit".into());
            }
            let mut seen = HashSet::new();
            let mut bytes = 0;
            let mut out = Vec::new();
            let mut add = |p: Page, offset: usize| -> Result<(), String> {
                if p.total != total
                    || p.offset != offset
                    || p.countries.len() != 100.min(total.saturating_sub(offset))
                {
                    return Err(
                        "Country pagination is inconsistent or incomplete. Retry later.".into(),
                    );
                }
                if !p.countries.is_empty() && !seen.insert(p.signature) {
                    return Err("Country pagination repeated a page without progress".into());
                }
                bytes += p.bytes;
                if bytes > MAX_BYTES {
                    return Err("Country data exceeds the 24 MiB total limit".into());
                }
                out.extend(p.countries);
                Ok(())
            };
            add(first, 0)?;
            for (offset, p) in [(100, b), (200, c)] {
                if offset < total {
                    add(p?, offset)?
                }
            }
            for offset in (300..total).step_by(100) {
                add(self.page(key, offset).await?, offset)?
            }
            if out.len() != total {
                return Err("Country list is incomplete".into());
            }
            Ok(out)
        })
        .await
        .map_err(|_| {
            "Country loading exceeded the 45 second deadline. Retry when ready.".to_string()
        })?
    }
}
pub(crate) fn contains(v: &Value, key: &str) -> bool {
    match v {
        Value::String(s) => s.contains(key),
        Value::Array(a) => a.iter().any(|v| contains(v, key)),
        Value::Object(m) => m.iter().any(|(k, v)| k.contains(key) || contains(v, key)),
        _ => false,
    }
}

struct Page {
    countries: Vec<Value>,
    total: usize,
    offset: usize,
    bytes: usize,
    signature: [u8; 32],
}
fn parse_page(bytes: &[u8], key: &str, offset: usize) -> Result<Page, String> {
    let body = js::parse(bytes)?;
    // A malicious upstream must not reflect a credential into records, keys or UI.
    if contains(&body, key) {
        return Err(
            "Country response contained private authentication data and was rejected".into(),
        );
    }
    let objects = body["data"]["objects"]
        .as_array()
        .ok_or("REST Countries returned an unexpected response shape")?;
    if objects.len() > 100 {
        return Err("REST Countries returned too many records in a page".into());
    }
    let meta = &body["data"]["meta"];
    let total = if meta.is_null() {
        objects.len()
    } else {
        meta["total"]
            .as_f64()
            .filter(|n| *n >= 0.0 && n.fract() == 0.0 && *n <= MAX_TOTAL as f64)
            .ok_or("Country response has an invalid or excessive total")? as usize
    };
    if !meta.is_null() {
        for (k, n) in [("offset", offset), ("limit", 100), ("count", objects.len())] {
            if let Some(v) = meta.get(k) {
                if v.as_f64() != Some(n as f64) {
                    return Err("Country pagination metadata is inconsistent".into());
                }
            }
        }
        if let Some(more) = meta.get("more") {
            if more.as_bool() != Some(offset + objects.len() < total) {
                return Err("Country pagination continuation is inconsistent".into());
            }
        }
    }
    let countries = objects
        .iter()
        .map(|v| {
            let mut c = v
                .as_object()
                .cloned()
                .ok_or("Country record must be an object")?;
            c.shift_remove("_match");
            c.shift_remove("_meta");
            let c = Value::Object(c);
            validate_country(&c)?;
            Ok(c)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let signature =
        Sha256::digest(js::stringify(&Value::Array(countries.clone())).as_bytes()).into();
    Ok(Page {
        countries,
        total,
        offset: if meta.is_null() { 0 } else { offset },
        bytes: bytes.len(),
        signature,
    })
}
/// Known presentation fields accept their documented shape and nullable/missing values.
/// Unknown JSON fields are retained. Malformed structures fail visibly, rather than guessing.
pub fn validate_country(c: &Value) -> Result<(), String> {
    if !c.is_object() {
        return Err("Country record must be an object".into());
    }
    for k in [
        "names",
        "codes",
        "flag",
        "area",
        "cars",
        "date",
        "demonyms",
        "number_format",
        "units",
        "memberships",
        "links",
    ] {
        if let Some(v) = c.get(k).filter(|v| !v.is_null()) {
            if !v.is_object() {
                return Err("Country response contains a malformed detail object".into());
            }
        }
    }
    for k in [
        "capitals",
        "languages",
        "currencies",
        "continents",
        "calling_codes",
        "tlds",
        "timezones",
        "borders",
    ] {
        if let Some(v) = c.get(k).filter(|v| !v.is_null()) {
            let a = v
                .as_array()
                .ok_or("Country response contains a malformed detail array")?;
            if a.len() > 256 {
                return Err("Country response detail array exceeds 256 entries".into());
            }
            for v in a {
                if ["capitals", "languages", "currencies"].contains(&k) {
                    if !v.is_object() {
                        return Err("Country detail entry must be an object".into());
                    }
                } else if !v.is_string() && !v.is_null() {
                    return Err("Country detail entry must be text".into());
                }
            }
        }
    }
    for path in [
        ["names", "common"],
        ["names", "official"],
        ["codes", "alpha_2"],
        ["codes", "alpha_3"],
        ["codes", "ccn3"],
        ["flag", "emoji"],
        ["flag", "description"],
    ] {
        if let Some(v) = js::get(c, &path).filter(|v| !v.is_null()) {
            if !v.is_string() {
                return Err("Country response has a malformed text field".into());
            }
        }
    }
    Ok(())
}
pub fn fingerprint(key: &str, scope: &str) -> String {
    let mut h = Sha256::new();
    h.update(scope.as_bytes());
    h.update([0]);
    h.update(key.as_bytes());
    format!("{:x}", h.finalize())
}
pub fn public_url(s: &str) -> Option<String> {
    if s.is_empty()
        || s.len() > 2048
        || s.chars().any(|c| c.is_control() || c.is_whitespace())
        || s.contains('\\')
        || s.contains(['<', '>', '"', '`'])
    {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    if (0..=31)
        .chain(std::iter::once(127))
        .any(|n| lower.contains(&format!("%{n:02x}")))
    {
        return None;
    }
    let u = url::Url::parse(s).ok()?;
    if !["http", "https"].contains(&u.scheme())
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some_and(|p| p != 80 && p != 443)
    {
        return None;
    }
    let h = u.host_str()?.to_ascii_lowercase();
    // Restrict numeric destinations and special-use/local domains. No DNS lookups during validation.
    if h.parse::<std::net::IpAddr>().is_ok()
        || h.starts_with('[')
        || !h.contains('.')
        || h.ends_with('.')
        || [
            "localhost",
            "local",
            "internal",
            "test",
            "invalid",
            "example",
            "onion",
        ]
        .iter()
        .any(|t| h == *t || h.ends_with(&format!(".{t}")))
    {
        return None;
    }
    Some(s.into())
}
pub fn flag_url(s: &str) -> Option<String> {
    let safe = public_url(s)?;
    let u = url::Url::parse(s).ok()?;
    if u.scheme() != "https"
        || u.port().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
        || !u.path().ends_with(".png")
        || !["flagcdn.com", "api.restcountries.com", "restcountries.com"].contains(&u.host_str()?)
    {
        return None;
    }
    Some(safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    const KEY: &str = "PUBLIC_SYNTHETIC_COUNTRY_TEST";
    async fn server(total: usize, status: u16) -> (Api, tokio::task::JoinHandle<Vec<usize>>) {
        server_mode(total, status, false).await
    }
    async fn server_mode(
        total: usize,
        status: u16,
        repeated: bool,
    ) -> (Api, tokio::task::JoinHandle<Vec<usize>>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = l.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let count = if status != 200 {
                3
            } else {
                3.max(total.div_ceil(100))
            };
            let mut offsets = vec![];
            for _ in 0..count {
                let (mut stream, _) = l.accept().await.unwrap();
                let mut bytes = vec![];
                loop {
                    let mut b = [0; 1024];
                    let n = stream.read(&mut b).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&b[..n]);
                    assert!(bytes.len() < 8192);
                    if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                assert!(request.to_ascii_lowercase().contains(&format!(
                    "authorization: bearer {}",
                    KEY.to_ascii_lowercase()
                )));
                let target = request.split_whitespace().nth(1).unwrap();
                let u = url::Url::parse(&format!("http://local{target}")).unwrap();
                assert_eq!(u.path(), "/countries/v5");
                assert_eq!(u.query_pairs().count(), 2);
                assert_eq!(
                    u.query_pairs().find(|(k, _)| k == "limit").unwrap().1,
                    "100"
                );
                let offset = u
                    .query_pairs()
                    .find(|(k, _)| k == "offset")
                    .unwrap()
                    .1
                    .parse::<usize>()
                    .unwrap();
                offsets.push(offset);
                let objects=(offset..total.min(offset+100)).map(|i|json!({"names":{"common":format!("Country {}",if repeated{i%100}else{i})},"_match":{"x":true},"_meta":{},"codes":{"alpha_3":format!("X{}",if repeated{i%100}else{i})}})).collect::<Vec<_>>();
                let body = if status == 200 {
                    json!({"data":{"objects":objects,"meta":{"total":total,"offset":offset,"count":objects.len(),"limit":100,"more":offset+objects.len()<total}}}).to_string()
                } else {
                    json!({"error":KEY}).to_string()
                };
                let headers=format!("HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nConnection: close\r\nLocation: http://127.0.0.1:1/leak\r\n\r\n",body.len());
                stream.write_all(headers.as_bytes()).await.unwrap();
                stream.write_all(body.as_bytes()).await.unwrap();
            }
            offsets
        });
        (Api::local(format!("http://{address}/countries/v5")), task)
    }
    #[tokio::test]
    async fn real_http_auth_pages_order_and_stripping() {
        let (api, task) = server(354, 200).await;
        let c = api.all(KEY).await.unwrap();
        assert_eq!(c.len(), 354);
        assert_eq!(c[353]["names"]["common"], "Country 353");
        assert!(c[0].get("_meta").is_none());
        let mut offsets = task.await.unwrap();
        offsets.sort();
        assert_eq!(offsets, vec![0, 100, 200, 300]);
    }
    #[tokio::test]
    async fn repeating_pages_fail_visibly() {
        let (api, t) = server_mode(200, 200, true).await;
        assert!(api.all(KEY).await.unwrap_err().contains("repeated"));
        t.await.unwrap();
    }
    #[tokio::test]
    async fn small_dataset_still_matches_source_speculative_offsets() {
        let (api, t) = server(1, 200).await;
        assert_eq!(api.all(KEY).await.unwrap().len(), 1);
        assert_eq!(t.await.unwrap().len(), 3);
    }
    #[tokio::test]
    async fn status_and_redirect_never_echo_body_or_follow() {
        for status in [301, 401, 403, 429, 500] {
            let (api, t) = server(10, status).await;
            let e = api.all(KEY).await.unwrap_err();
            assert!(!e.contains(KEY));
            assert!(!e.contains("127.0.0.1"));
            assert_eq!(t.await.unwrap().len(), 3);
        }
    }
    #[test]
    fn malformed_limits_and_secret_reflection() {
        for body in [
            json!({}),
            json!({"data":{"objects":null}}),
            json!({"data":{"objects":[true]}}),
            json!({"data":{"objects":[{}],"meta":{"total":2001}}}),
            json!({"data":{"objects":[{}],"meta":{"total":1,"offset":100}}}),
            json!({"data":{"objects":[{}],"meta":{"total":1,"count":2}}}),
            json!({"data":{"objects":[{}],"meta":{"total":1,"more":true}}}),
            json!({"data":{"objects":[{"unknown":KEY}]}}),
            json!({"data":{"objects":[{"names":true}]}}),
            json!({"data":{"objects":vec![json!({});101]}}),
        ] {
            assert!(parse_page(body.to_string().as_bytes(), KEY, 0).is_err())
        }
        assert!(parse_page(&vec![b' '; 8 * 1024 * 1024 + 1], KEY, 0).is_err());
        let p = parse_page(br#"{"data":{"objects":[{"unknown":1}]}}"#, KEY, 0).unwrap();
        assert_eq!(p.total, 1);
    }
    #[tokio::test]
    async fn body_limit_and_deadline() {
        for slow in [false, true] {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            let t = tokio::spawn(async move {
                let (mut s, _) = l.accept().await.unwrap();
                let mut b = [0; 8192];
                let _ = s.read(&mut b).await;
                if slow {
                    tokio::time::sleep(Duration::from_millis(700)).await;
                } else {
                    let _ = s
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n")
                        .await;
                }
            });
            let api = Api::local(format!("http://{addr}/countries/v5"));
            assert!(api.page(KEY, 0).await.is_err());
            t.await.unwrap();
        }
    }
}

#[cfg(test)]
mod tls_tests {
    use super::*;
    #[tokio::test]
    async fn real_tls_rejects_untrusted_local_certificate_without_leaking_key() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");
        std::fs::create_dir_all(&root).unwrap();
        let temp = tempfile::tempdir_in(root).unwrap();
        let key = temp.path().join("mock-key.pem");
        let cert = temp.path().join("mock-cert.pem");
        let output = std::process::Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-keyout",
            ])
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .output()
            .expect("openssl is required for the local TLS QA test");
        assert!(output.status.success());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut child = tokio::process::Command::new("python3")
            .arg("-c").arg("import socket,ssl,sys\nc=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);c.load_cert_chain(sys.argv[2],sys.argv[3])\ns=socket.socket();s.bind(('127.0.0.1',int(sys.argv[1])));s.listen()\nwhile True:\n a,_=s.accept()\n try:c.wrap_socket(a,server_side=True).close()\n except ssl.SSLError:a.close()")
            .arg(port.to_string()).arg(&cert).arg(&key)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut ready = false;
        for _ in 0..50 {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(ready);
        let api = Api::new(format!("https://127.0.0.1:{port}/countries/v5")).unwrap();
        let e = api
            .page("PUBLIC_SYNTHETIC_TLS_TEST", 0)
            .await
            .err()
            .unwrap();
        assert!(e.contains("securely"));
        assert!(!e.contains("PUBLIC_SYNTHETIC"));
        child.kill().await.unwrap();
        child.wait().await.unwrap();
    }
}
