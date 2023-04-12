#!/bin/bash

set -x

whoami
pwd
ls -latr ~/.aws/

echo "Inside Docker - /opt/docker/kickstart_runner.sh"

mkdir -p ~/.aws/
#cat $PUBLIC_SSH_KEY > ~/.ssh/id_rsa.pub

aws sts get-caller-identity
# mv /src/id_rsa ~/.ssh/id_rsa
# mv /src/id_rsa.pub ~/.ssh/id_rsa.pub

# chmod 400 ~/.ssh/id_rsa*

tree ~/.aws/
