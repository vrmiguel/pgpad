use std::{
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::bail;

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

fn db_path() -> PathBuf {
    env::var_os("PGPAD_WEB_DB")
        .map(PathBuf::from)
        .unwrap_or_else(pgpad_web::default_db_path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let static_dir = static_dir();
    let index = static_dir.join("index.html");
    if !index.exists() {
        bail!(
            "Could not find {}, run `npm run build` first or set PGPAD_WEB_DIST",
            index.display()
        )
    }

    let addr = bind_addr()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let state = pgpad_web::WebState::new(db_path())?;

    println!("Serving pgpad from {}", static_dir.display());
    println!("Listening on http://{addr}");

    axum::serve(listener, pgpad_web::router(static_dir, state)).await?;

    Ok(())
}
