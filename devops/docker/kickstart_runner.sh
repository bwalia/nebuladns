#!/bin/bash

set -x

#   sleep 120

whoami
pwd

ls -latr ~/.aws/
    #   stat ~/.aws/credentials
    #   cat ~/.aws/credentials
#tree ~/.aws/

echo "Inside Docker - devops/docker/kickstart_runner.sh"

aws sts get-caller-identity

# # mv /src/id_rsa ~/.ssh/id_rsa
# # mv /src/id_rsa.pub ~/.ssh/id_rsa.pub
# # chmod 400 ~/.ssh/id_rsa*

# # 
