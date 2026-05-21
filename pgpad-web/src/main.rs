use std::{
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
};

fn default_static_dir() -> PathBuf {
    let cwd_dist = env::current_dir()
        .map(|cwd| cwd.join("dist"))
        .unwrap_or_else(|_| PathBuf::from("dist"));

    if cwd_dist.exists() {
        return cwd_dist;
    }

    Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist")
}

fn static_dir() -> PathBuf {
    env::var_os("PGPAD_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(default_static_dir)
}

fn bind_addr() -> Result<SocketAddr, std::net::AddrParseError> {
    env::var("PGPAD_WEB_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let static_dir = static_dir();
    let index = static_dir.join("index.html");
    if !index.exists() {
        return Err(format!(
            "Could not find {}, run `npm run build` first or set PGPAD_WEB_DIST",
            index.display()
        )
        .into());
    }

    let addr = bind_addr()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("Serving pgpad from {}", static_dir.display());
    println!("Listening on http://{addr}");

    axum::serve(listener, pgpad_web::router(static_dir)).await?;

    Ok(())
}
