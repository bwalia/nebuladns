#!/bin/bash

export NVM_DIR=$HOME/.nvm;
source $NVM_DIR/nvm.sh;

NVM_BIN=nvm
NODE_VERSION=16.18.1
NPM_BIN=$(which npm)

${NVM_BIN} use ${NODE_VERSION}

${NPM_BIN} install
${NPM_BIN} run build

HOST_ENDPOINT_UNSECURE_URL="http://localhost:3000/"
HOST_ENDPOINT_SECURE_URL="https://localhost:9943/"

if [ $targetEnv == "int" ]; then
HOST_ENDPOINT_UNSECURE_URL="http://int.odincm.com"
fi

curl -I $HOST_ENDPOINT_SECURE_URL
os_type=$(uname -s)

if [ "$os_type" = "Darwin" ]; then
open $HOST_ENDPOINT_SECURE_URL
fi

${NPM_BIN} run start

