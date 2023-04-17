#!/bin/bash
set -x

IMAGE_REGISTRY=$AWS_ACCOUNT_NO.dkr.ecr.eu-west-2.amazonaws.com
if [ -z "${TARGET_ENV}" ]; then
    TARGET_ENV=test
fi
if [ -z "${IMAGE_NAME}" ]; then
    IMAGE_NAME=odincm
fi
if [ -z "${IMAGE_TAG}" ]; then
    IMAGE_TAG=latest
fi
if [ -z "${IMAGE_REGISTRY}" ]; then
    exit 1
fi
if [ -z "${AWS_ACCOUNT_NO}" ]; then
    exit 1
fi
echo "TARGET_ENV=$TARGET_ENV" >> $GITHUB_ENV
echo "IMAGE_NAME=$IMAGE_NAME" >> $GITHUB_ENV
echo "IMAGE_TAG=$IMAGE_TAG" >> $GITHUB_ENV

aws ecr get-login-password --region eu-west-2 | docker login --username AWS --password-stdin $AWS_ACCOUNT_NO.dkr.ecr.eu-west-2.amazonaws.com
docker build -f devops/docker/Dockerfile-nuxt-server -t $IMAGE_NAME . --no-cache
docker tag $IMAGE_NAME $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
docker push $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
