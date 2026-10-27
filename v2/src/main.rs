use country_lookup_native::Adapter;
#[tokio::main(worker_threads = 2)]
async fn main() {
    if tezbar_extension_sdk::serve(Adapter::default())
        .await
        .is_err()
    {
        eprintln!("Country Lookup protocol ended with an error");
        std::process::exit(1);
    }
}
