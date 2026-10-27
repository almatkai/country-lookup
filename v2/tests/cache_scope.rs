use country_lookup_native::{data, Adapter};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tezbar_extension_sdk::{Extension, ExtensionPaths, TestContext};
const KEY: &str = "PUBLIC_SYNTHETIC_CACHE_SCOPE";
fn context(root: &std::path::Path) -> TestContext {
    TestContext::new(
        ExtensionPaths {
            data_dir: root.into(),
            package_dir: root.into(),
            temp_dir: root.into(),
        },
        &[],
    )
}
fn seed(ctx: &TestContext, acquired: u64) {
    let dir = ctx.context.paths.data_dir.join("country-lookup-cache");
    std::fs::create_dir_all(&dir).unwrap();
    let fingerprint = data::fingerprint(KEY, &ctx.context.paths.data_dir.to_string_lossy());
    std::fs::write(dir.join("storage.json"),json!({"unknown":"preserved","entry":{"fingerprint":fingerprint,"time":acquired,"countries":[{"names":{"common":"Scoped synthetic country"}}]}}).to_string()).unwrap();
}
async fn open(adapter: &Adapter, ctx: &TestContext, id: &str, key: &str) -> Value {
    let p = json!({"sessionId":id,"preferences":{"apiKey":key,"entriesWithoutIsoCode":"show"}});
    let v = adapter
        .handle(
            &ctx.context,
            "index",
            "extension:run-command",
            &p,
            &mut json!({}),
        )
        .await
        .unwrap();
    assert!(!v.to_string().contains("PUBLIC_SYNTHETIC"));
    v
}
#[tokio::test]
async fn actual_cache_is_key_profile_and_expiry_scoped_and_survives_new_adapter() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");
    std::fs::create_dir_all(&root).unwrap();
    let first = tempfile::tempdir_in(&root).unwrap();
    let second = tempfile::tempdir_in(&root).unwrap();
    let a = context(first.path());
    let b = context(second.path());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    seed(&a, now);
    let adapter = Adapter::default();
    let valid = open(&adapter, &a, "same", KEY).await;
    assert_eq!(
        valid["root"]["children"][0]["props"]["title"],
        "Scoped synthetic country"
    );
    for (ctx, id, key) in [
        (&a, "other-key", "PUBLIC_SYNTHETIC_OTHER_KEY"),
        (&b, "other-profile", KEY),
    ] {
        let v = open(&adapter, ctx, id, key).await;
        assert_eq!(
            v["root"]["children"][0]["props"]["title"],
            "Could not load countries"
        );
        assert!(v["root"]["children"][0]["props"]["description"]
            .as_str()
            .unwrap()
            .contains("placeholder"));
    }
    let restarted = open(&Adapter::default(), &a, "restart", KEY).await;
    assert_eq!(
        restarted["root"]["children"][0]["props"]["title"],
        "Scoped synthetic country"
    );
    for (stamp, id) in [(now - 4 * 60 * 60 - 1, "expired"), (now + 600, "future")] {
        seed(&a, stamp);
        let v = open(&Adapter::default(), &a, id, KEY).await;
        assert_eq!(
            v["root"]["children"][0]["props"]["title"],
            "Could not load countries"
        );
    }
    assert!(a.calls().is_empty());
    assert!(b.calls().is_empty());
}
