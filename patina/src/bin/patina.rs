#[tokio::main(flavor = "current_thread")]
async fn main() {
    patina::server_cli(None).await;
}
