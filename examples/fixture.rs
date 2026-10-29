#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = rustload::fixture::spawn(8080).await?;
    println!(
        "Local RustLoad fixture: http://{} (Ctrl-C to stop)",
        fixture.address
    );
    tokio::signal::ctrl_c().await?;
    fixture.stop.cancel();
    Ok(())
}
