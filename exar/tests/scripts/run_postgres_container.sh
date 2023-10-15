#!/bin/sh

POSTGRES_USER=postgres
POSTGRES_PASSWORD=2XWVhtvi
POSTGRES_DB=experiment_base
PORT=16378

# cd to tests folder (parent of the location of this script)
cd "${0%/*}" && cd ..
TEST_DATA_DIR=$PWD/data

# delete old container if it is running. makes it easier to run unit tests locally by just re-running this script
docker rm -f exar-postgres-test

# run postgres docker container and create database
docker run -d --name exar-postgres-test -p $PORT:5432 -e POSTGRES_PASSWORD=$POSTGRES_PASSWORD -e POSTGRES_DB=$POSTGRES_DB postgres:latest

if [ $? -ne 0 ]; then
    echo "Failed to launch postgres docker container" && exit 1
fi