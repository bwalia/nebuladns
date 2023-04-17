#!/bin/bash
set -x

IMAGE_REGISTRY=$AWS_ACCOUNT_NO.dkr.ecr.eu-west-2.amazonaws.com
if [ -z "${TARGET_ENV}" ]; then
TARGET_ENV=k3s2
fi
aws ecr get-login-password --region eu-west-2 | docker login --username AWS --password-stdin $AWS_ACCOUNT_NO.dkr.ecr.eu-west-2.amazonaws.com
docker build -f devops/docker/Dockerfile-nuxt-server -t $IMAGE_NAME . --no-cache
docker tag $IMAGE_NAME $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
docker push $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
