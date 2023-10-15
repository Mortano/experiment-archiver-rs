#!/bin/sh

POSTGRES_USER=postgres
POSTGRES_PASSWORD=2XWVhtvi
POSTGRES_DB=experiment_base
PORT=16378

# run postgres docker container and create database
docker run -d --name exar-postgres-test -p $PORT:5432 -e POSTGRES_PASSWORD=$POSTGRES_PASSWORD -e POSTGRES_DB=$POSTGRES_DB postgres:latest

if [ $? -ne 0 ]; then
    echo "Failed to launch postgres docker container" && exit 1
fi

echo "Waiting for database to be ready..."
sleep 2

# setup env vars for tests
export PSQL_USER=$POSTGRES_USER
export PSQL_PWD=$POSTGRES_PASSWORD
export PSQL_HOST=0.0.0.0
export PSQL_PORT=$PORT
export PSQL_DBNAME=$POSTGRES_DB
export RUST_BACKTRACE=1

# execute integration tests
cargo test --release --test integration

# shutdown container
docker rm -f exar-postgres-test