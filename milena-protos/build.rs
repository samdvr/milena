fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/cache_server.proto")?;
    tonic_build::compile_protos("proto/router_server.proto")?;
    Ok(())
}
