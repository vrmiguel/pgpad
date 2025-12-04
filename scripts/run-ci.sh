#!/bin/bash
set -e

# Start Postgres
echo "Starting Postgres..."
if [ -x /etc/init.d/postgresql ]; then
    /etc/init.d/postgresql start
else
    service postgresql start
fi

# Wait for Postgres to be ready
echo "Waiting for Postgres..."
until sudo -u postgres psql -c '\q'; do
  echo "Postgres is unavailable - sleeping"
  sleep 1
done
echo "Postgres is up!"

echo "Running Node checks..."
npm run format:check
npx eslint .

echo "Running Rust checks..."
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked -- --nocapture
