use std::fs;
use std::path::Path;

fn rewrite_generated_server_traits(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let original = fs::read_to_string(path)?;
    let mut rewritten = original.clone();

    rewritten = rewritten.replace(
        r#"    #[async_trait]
    pub trait HyperdexAdmin: std::marker::Send + std::marker::Sync + 'static {
        async fn create_space(
            &self,
            request: tonic::Request<super::CreateSpaceRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CreateSpaceResponse>,
            tonic::Status,
        >;
    }
"#,
        r#"    pub trait HyperdexAdmin: std::marker::Send + std::marker::Sync + 'static {
        fn create_space(
            &self,
            request: tonic::Request<super::CreateSpaceRequest>,
        ) -> BoxFuture<tonic::Response<super::CreateSpaceResponse>, tonic::Status>;
    }
"#,
    );
    rewritten = rewritten.replace(
        r#"    #[async_trait]
    pub trait HyperdexClient: std::marker::Send + std::marker::Sync + 'static {
        async fn put(
            &self,
            request: tonic::Request<super::PutRequest>,
        ) -> std::result::Result<tonic::Response<super::PutResponse>, tonic::Status>;
        async fn get(
            &self,
            request: tonic::Request<super::GetRequest>,
        ) -> std::result::Result<tonic::Response<super::GetResponse>, tonic::Status>;
    }
"#,
        r#"    pub trait HyperdexClient: std::marker::Send + std::marker::Sync + 'static {
        fn put(
            &self,
            request: tonic::Request<super::PutRequest>,
        ) -> BoxFuture<tonic::Response<super::PutResponse>, tonic::Status>;
        fn get(
            &self,
            request: tonic::Request<super::GetRequest>,
        ) -> BoxFuture<tonic::Response<super::GetResponse>, tonic::Status>;
    }
"#,
    );
    rewritten = rewritten.replace(
        r#"    #[async_trait]
    pub trait InternodeTransport: std::marker::Send + std::marker::Sync + 'static {
        async fn send(
            &self,
            request: tonic::Request<super::InternodeRpcRequest>,
        ) -> std::result::Result<
            tonic::Response<super::InternodeRpcResponse>,
            tonic::Status,
        >;
    }
"#,
        r#"    pub trait InternodeTransport: std::marker::Send + std::marker::Sync + 'static {
        fn send(
            &self,
            request: tonic::Request<super::InternodeRpcRequest>,
        ) -> BoxFuture<tonic::Response<super::InternodeRpcResponse>, tonic::Status>;
    }
"#,
    );

    if rewritten == original {
        return Err(format!("generated server trait rewrite did not match {}", path.display()).into());
    }

    fs::write(path, rewritten)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    tonic_build::configure().compile_protos(&["proto/hyperdex.proto"], &["proto"])?;
    let out_dir = std::env::var("OUT_DIR")?;
    rewrite_generated_server_traits(&Path::new(&out_dir).join("hyperdex.v1.rs"))?;
    Ok(())
}
