#!/bin/bash

BUILD_IMAGE_APP=docker
IMAGE_TAG=dev

${BUILD_IMAGE_APP} build -f $(pwd)/devops/docker/Dockerfile-nuxt-server -t bwalia/odincm . --no-cache
${BUILD_IMAGE_APP} tag bwalia/odincm registry.workstation.co.uk/odincm:${IMAGE_TAG}
${BUILD_IMAGE_APP} push registry.workstation.co.uk/odincm:${IMAGE_TAG}



