<div align="center">
  <img align="center" width="128px" src="https://github.com/user-attachments/assets/28f39044-185c-4750-b2e2-21f56abc773a" />
	<h1 align="center"><b>pgpad</b></h1>
	<p align="center">
		[WIP] A straightforward cross-platform database client
  </p>
</div>

<img align="center" width="1624" height="1056" alt="image" src="https://github.com/user-attachments/assets/fecbe1e2-d0a5-46cc-8843-78b25a509a3f" />

### What is it?

- A lightweight, snappy tool for everyday queries
  - Quick startup: loads up in less than a second in my machine.
  - Small memory footprint
  - Small bundle size
- Most importantly, pgpad is _free_, and will always be. That includes not ever having a "Community Edition", pop-ups that ask you for an upgrade, or anything of the sort.

### What is it _not_?

- A fully-fledged professional DB management system like DBeaver.

### Supported databases

|       Database       |        Status         |                                                         Note                                                          |                                          Driver                                           |
| :------------------: | :-------------------: | :-------------------------------------------------------------------------------------------------------------------: | :---------------------------------------------------------------------------------------: |
|      PostgreSQL      | Implemented, primary  |                                    Implemented, most used by authors. Unit-tested.                                    |            [`tokio-postgres`](https://github.com/rust-postgres/rust-postgres)             |
|        SQLite        |      Implemented      |                                               Implemented, unit-tested.                                               |                    [`rusqlite`](https://github.com/rusqlite/rusqlite)                     |
|     CockroachDB      |      Implemented      |                Implemented due to the Postgres Wire Protocol. No CockroachDB-specific tests currently                 |            [`tokio-postgres`](https://github.com/rust-postgres/rust-postgres)             |
|        MySQL         |        Planned        |                                                                                                                       |                                            das                                            |
| Microsoft SQL Server |        Planned        |                                                                                                                       |                                                                                           |
|        Oracle        |        Planned        |                                                                                                                       |                 [`mysql`](https://github.com/blackbeam/rust-mysql-simple)                 |
|      Clickhouse      |        Planned        |                                                                                                                       |                [`clickhouse`](https://github.com/ClickHouse/clickhouse-rs)                |
|      SQLCipher       |        Planned        |                                                                                                                       |                    [`rusqlite`](https://github.com/rusqlite/rusqlite)                     |
|        DuckDB        |        Planned        |                                                                                                                       | [`duckdb`](<[https://github.com/rusqlite/rusqlite](https://github.com/duckdb/duckdb-rs)>) |
|       MongoDB        | Not currently planned |                                Would require some refactors to accomodate a NoSQL DBMS                                |                                                                                           |
|       MariaDB        |                       | Rust lacks a dedicated MariaDB driver. As it stands, we'd be able to support MariaDB only through MySQL compatibility |                                                                                           |

#### Operating systems

`pgpad` supports Windows (7+), macOS (10.15+), and Linux (must have `libwebkit2gtk` 4.1 or higher).

## Building

### Requirements

- A relatively recent build of `npm`
- The Rust toolchain, with a minimum version of 1.85

### Setup

#### 1. Install dependencies

```
npm install
```

#### Build executable

```
npm run tauri build
```

#### To start the dev server

```
npm run tauri dev
```

## Docker

- Build installers without local toolchains using the multi-stage `Dockerfile`.
- Optional Linux runtime image to run the GUI via X11/Wayland.

### Build installers (artifacts)

- Using Docker directly:
  - `docker build --target artifacts --output type=local,dest=./out .`
  - Bundles appear under `./out/bundle` (AppImage/deb/rpm on Linux).

- Using Compose profile:
  - `docker compose --profile build up -d pgpad-build`
  - `docker cp $(docker compose ps -q pgpad-build):/out/bundle ./bundle`
  - `docker compose down pgpad-build`

### Run on Linux (X11/Wayland)

- Allow local X11 connections (host): `xhost +local:`
- Start the container:
  - `docker compose --profile run-linux up --build pgpad`
- Environment and mounts:
  - X11: `-v /tmp/.X11-unix:/tmp/.X11-unix -e DISPLAY=$DISPLAY`
  - Wayland: `-v $XDG_RUNTIME_DIR:/XDG_RUNTIME_DIR -e WAYLAND_DISPLAY=$WAYLAND_DISPLAY`
  - App data: `-v $HOME/.local/share/pgpad:/home/pgpad/.local/share/pgpad`
  - Certificates: `-v ./certs:/opt/pgpad/certs`

### CI/CD

- GitHub Actions workflow builds bundles via Docker and uploads to releases.
- Tags push a `linux-runtime-<tag>` image to GHCR: `ghcr.io/<owner>/<repo>`.

### Security

- No certs or secrets are baked into images; mount via volumes.
- Runtime runs as non-root `pgpad` user.

## A work in progress!

Feel free to open issues for bug reports and feature requests.
