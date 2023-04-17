#!/bin/bash

IMAGE_REGISTRY=${{ env.AWS_ACCOUNT_NO }}.dkr.ecr.eu-west-2.amazonaws.com
echo "BRANCH_REF=main" >> $GITHUB_ENV
echo "IMAGE_REGISTRY=${IMAGE_REGISTRY} >> $GITHUB_ENV
echo "IMAGE_NAME=${{ env.IMAGE_NAME }}" >> $GITHUB_ENV
echo "IMAGE_TAG=${{ env.IMAGE_TAG }}" >> $GITHUB_ENV
TARGET_ENV=${{ env.TARGET_ENV }}
if [ -z "${{ env.TARGET_ENV }}" ]; then
TARGET_ENV=k3s2
fi
echo "TARGET_ENV=${TARGET_ENV}" >> $GITHUB_ENV
TARGET_CLUSTER=${{ env.TARGET_CLUSTER }}
if [ -z "${{ env.TARGET_CLUSTER }}" ]; then
TARGET_CLUSTER=k3s2
fi
echo "TARGET_CLUSTER=${TARGET_CLUSTER}" >> $GITHUB_ENV
aws ecr get-login-password --region eu-west-2 | docker login --username AWS --password-stdin ${{ env.AWS_ACCOUNT_NO }}.dkr.ecr.eu-west-2.amazonaws.com
docker build -f devops/docker/Dockerfile-nuxt-server -t $IMAGE_NAME . --no-cache
docker tag $IMAGE_NAME $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
docker push $IMAGE_REGISTRY/$IMAGE_NAME:$IMAGE_TAG
