#!/bin/bash

set -x

aws ecr get-login-password --region eu-west-2 | docker login --username AWS --password-stdin 123154119074.dkr.ecr.eu-west-2.amazonaws.com

kubectl version
helm version

IMAGE_REGISTRY=123154119074.dkr.ecr.eu-west-2.amazonaws.com
IMAGE_NAME=odincm
IMAGE_TAG=latest

#docker build -f devops/docker/Dockerfile-nuxt-server --build-arg -t $IMAGE_NAME . --no-cache
docker build -f devops/docker/Dockerfile-nuxt-server -t $IMAGE_NAME . --no-cache
docker tag $IMAGE_NAME $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
docker push $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG

#sleep 120
#whoami
#pwd

#   ls -latr ~/.aws/

#   stat ~/.aws/credentials
#   cat ~/.aws/credentials
#   tree ~/.aws/

#echo "Inside Docker - devops/docker/kickstart_runner.sh"

#   aws sts get-caller-identity
#   security risk

# # mv /src/id_rsa ~/.ssh/id_rsa
# # mv /src/id_rsa.pub ~/.ssh/id_rsa.pub
# # chmod 400 ~/.ssh/id_rsa*

#   ls -A

