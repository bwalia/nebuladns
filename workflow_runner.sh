#!/bin/bash

set -x

if ! docker info > /dev/null 2>&1; then
  echo "This script uses docker, and it isn't running - please start docker and try again!"
# if mac   say "This script uses docker, and it isn't running - please start docker and try again!"
  exit 1
fi

if [ -z "$1" ];
then
  echo "action name is not set"
  exit 1
fi

cmd_action=$1

if [ -z "$2" ];
then
  echo "replica count is not set default is 1"
  REPLICA_COUNT="1"
else
  REPLICA_COUNT=$2
fi

if [ -z "$3" ];
then
  echo "aws region name is not set default is london"
  AWS_REGION_NAME="london"
else
  AWS_REGION_NAME=$3
fi

if [ -z "$4" ];
then
  echo "kubeconfig is not set kubectl or helm commands will not work"
  KUBECONFIG=""
else
  KUBECONFIG=$4
fi

if [ -z ${cmd_action} ];
then
  cmd_action="update"
fi

if [ -z ${cmd_action} ];
then
  cmd_action="update"
fi

if test -z "$cmd_action" 
then
      echo "\$cmd_action is empty"
      cmd_action="update"
else
      echo "\$cmd_action is NOT empty"
fi

if [ -z ${REPLICA_COUNT} ];
then
  REPLICA_COUNT="1"
fi

if test -z "$REPLICA_COUNT" 
then
      echo "\$REPLICA_COUNT is empty"
      REPLICA_COUNT="1"
else
      echo "\$REPLICA_COUNT is NOT empty"
fi

if [ -z ${DOCKER_IMAGE_ID} ];
then
  DOCKER_IMAGE_ID="odincm"
fi

if test -z "$DOCKER_IMAGE_ID" 
then
      echo "\$DOCKER_IMAGE_ID is empty"
      DOCKER_IMAGE_ID="odincm"
else
      echo "\$DOCKER_IMAGE_ID is NOT empty"
fi

if [ -z ${TARGET_ENV} ];
then
  TARGET_ENV="prod"
fi

if [ -z ${AWS_DEFAULT_REGION} ];
then
  AWS_DEFAULT_REGION="eu-west-2"
fi

if [ -z ${TF_STATE_BUCKET} ];
then
  TF_STATE_BUCKET="pipeline-tf-state"
fi

if [ "$AWS_REGION_NAME" == "dublin" ]; then
  AWS_DEFAULT_REGION="eu-west-1"
fi

if [ "$AWS_REGION_NAME" == "london" ]; then
  AWS_DEFAULT_REGION="eu-west-2"
fi
if [ -z "$3" ];
then
  echo "region cannot be set"
  exit 1
fi

# whoami
# pwd
# ls -la

# mkdir -p /home/runner/.kube
# mkdir -p ~/.aws/

# echo "[default]" > ~/.aws/credentials
# echo "aws_access_key_id = ${AWS_ACCESS_KEY_ID}" >> ~/.aws/credentials
# echo "aws_secret_access_key = ${AWS_SECRET_ACCESS_KEY}" >> ~/.aws/credentials
# echo "region = ${AWS_DEFAULT_REGION}" >> ~/.aws/credentials
# echo "output = json" >> ~/.aws/credentials
# stat ~/.aws/credentials
# cat ~/.aws/credentials

# export AWS_ACCOUNT_NO=${AWS_ACCOUNT_NO}
# export AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}
# export AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}
# export AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION}

#   export "AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}"
# echo "$2" > /home/runner/id_rsa.pub
# echo "$3" > /home/runner/id_rsa
# echo "$4" > /home/runner/.kube/k3s.yaml
# chmod 600 /home/runner/.kube/k3s.yaml

# export KUBECONFIG=/home/runner/.kube/k3s.yaml

workdflow_build_run_in_docker_container () {

echo 'Workdflow run in docker container'

DOCKER_IMAGE_NAME="my_custom_workflow_runner"

#echo "${EC2_SSH_PRIVATE_KEY}"
#docker system prune -f

DOCKER_IMAGE_CACHE="--no-cache"         #DOCKER_IMAGE_CACHE=""

docker build -f devops/docker/Dockerfile_runner -t ${DOCKER_IMAGE_NAME} . ${DOCKER_IMAGE_CACHE}

#echo "${EC2_SSH_PRIVATE_KEY}"
#docker system prune -f

# curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
# unzip awscliv2.zip
# sudo ./aws/install
# which aws
# aws sts get-caller-identity security risk

# docker build -f devops/docker/Dockerfile-nuxt-server -t ${DOCKER_IMAGE_ID} . ${DOCKER_IMAGE_CACHE}
# docker tag ${DOCKER_IMAGE_ID} 123154119074.dkr.ecr.eu-west-2.amazonaws.com/${DOCKER_IMAGE_ID}:latest
# aws ecr get-login-password --region eu-west-2 | docker login --username AWS --password-stdin  123154119074.dkr.ecr.eu-west-2.amazonaws.com
# docker push 123154119074.dkr.ecr.eu-west-2.amazonaws.com/${DOCKER_IMAGE_ID}:latest
# docker run \
# -e "AWS_ACCESS_KEY_ID=" \
# -e "AWS_SECRET_ACCESS_KEY=" \
# -e "AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION}" \
# -e "AWS_PROFILE=default" \
# -e "AWS_REGION_NAME=${AWS_REGION_NAME}" \
# -v "AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}" \
# -v "AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}" \
# -v "AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION}" \
# -v "AWS_PROFILE=default" $DOCKER_IMAGE_NAME
docker run -e AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID} -e AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY} -e AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION} -e AWS_PROFILE=default $DOCKER_IMAGE_NAME
}

workdflow_build_run_in_docker_container

