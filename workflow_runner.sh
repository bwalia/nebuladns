#!/bin/bash

set -x
"${{ env.cmd_action }}" "${{ env.REPLICA_COUNT }}" "${{ env.AWS_REGION_NAME }}" "${{ env.KUBE_CONFIG_DATA_K3S2 }}"

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
      REPLICA_COUNT="syn-pop-prod"
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

if [ -z ${POP_ENV} ];
then
  POP_ENV="prod"
fi

if [ -z ${POP_REGION} ];
then
  POP_REGION="eu-west-2"
fi

if [ -z ${POP_STATE_BUCKET} ];
then
  POP_STATE_BUCKET="syn-cloud-tf-state"
fi

if [ -z "$2" ];
then
  echo "your ssh public key is not set"
  exit 1
fi

if [ -z "$3" ];
then
  echo "your ssh private key is not set"
  exit 1
fi

if [ -z "$4" ];
then
  echo "tls api cert is not set"
  exit 1
fi

if [ -z "$5" ];
then
  echo "tls api cert key is not set"
  exit 1
fi

if [ -z "$6" ];
then
  echo "api token sign key is not set"
  exit 1
fi

if [ -z "$7" ];
then
  echo "ec2 instance type is not set"
  EC2_INSTANCE_TYPE="t2.nano"
else
  EC2_INSTANCE_TYPE=$7
fi

if [ -z "$8" ];
then
  echo "ec2 instance count is not set default is 2"
  EC2_INSTANCE_COUNT_PER_AZ="1"
else
  EC2_INSTANCE_COUNT_PER_AZ=$8
fi

if [ -z "$9" ];
then
  echo "aws region name is not set default is london"
  AWS_REGION_NAME="london"
else
  AWS_REGION_NAME=$9
fi

if [ "$AWS_REGION_NAME" == "dublin" ]; then
  POP_REGION="eu-west-1"
fi

if [ "$AWS_REGION_NAME" == "london" ]; then
  POP_REGION="eu-west-2"
fi

if [ -z "$10" ];
then
  echo "ec2 key at the time of launch is not set"
  exit 1
else
  echo "ec2 key at the time of launch is set"
fi

# echo "$2" > terraform_common/src/id_rsa.pub
# echo "$3" > terraform_common/src/id_rsa
# echo "$4" > terraform_common/src/api_ssl_cert.crt
# echo "$5" > terraform_common/src/api_ssl_cert.key
# echo "$6" > terraform_common/src/api_sign.key
# echo "$10" > terraform_common/src/ec2.pem

workdflow_build_run_in_docker_container () {

echo 'Workdflow run in docker container'

DOCKER_IMAGE_NAME="$DOCKER_IMAGE_ID"_"$AWS_REGION_NAME"_"$cmd_action"

#echo "${EC2_SSH_PRIVATE_KEY}"
#docker system prune -f

DOCKER_IMAGE_CACHE="--no-cache"         #DOCKER_IMAGE_CACHE=""

#echo "${EC2_SSH_PRIVATE_KEY}"
#docker system prune -f

curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install
which aws

docker build -f devops/docker/Dockerfile-nuxt-server -t ${DOCKER_IMAGE_NAME} . ${DOCKER_IMAGE_CACHE}
docker tag ${DOCKER_IMAGE_NAME} 123154119074.dkr.ecr.eu-west-2.amazonaws.com/odincm:latest
aws ecr get-login-password --region eu-west-2 | docker login --username AWS --password-stdin  123154119074.dkr.ecr.eu-west-2.amazonaws.com
docker push 123154119074.dkr.ecr.eu-west-2.amazonaws.com/odincm:latest

# docker run \
# -e "AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}" \
# -e "AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}" \
# -e "AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION}" \
# -e "AWS_PROFILE=${AWS_PROFILE}" \
# -e "POP_ENV=${POP_ENV}" \
# -e "REPLICA_COUNT=${REPLICA_COUNT}" \
# -e "cmd_action=${cmd_action}" \
# -e "POP_STATE_BUCKET=${POP_STATE_BUCKET}" \
# -e "AWS_PROFILE=default" \
# -e "AWS_DEFAULT_REGION=${POP_REGION}" \
# -e "EC2_INSTANCE_TYPE=${EC2_INSTANCE_TYPE}" \
# -e "EC2_INSTANCE_COUNT_PER_AZ=${EC2_INSTANCE_COUNT_PER_AZ}" \
# -e "AWS_REGION_NAME=${AWS_REGION_NAME}" \
# -v "AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID}" \
# -v "AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY}" \
# -v "AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION}" \
# -v "AWS_PROFILE=${AWS_PROFILE}" \
# -v "REPLICA_COUNT=${REPLICA_COUNT}" \
# -v "POP_ENV=${POP_ENV}" \
# -v "cmd_action=${cmd_action}" \
# -v "POP_STATE_BUCKET=${POP_STATE_BUCKET}" \
# -v "AWS_PROFILE=default" \
# -v "AWS_DEFAULT_REGION=${POP_REGION}" \
# -v "EC2_INSTANCE_COUNT_PER_AZ=${EC2_INSTANCE_COUNT_PER_AZ}" \
# -v "EC2_INSTANCE_TYPE=${EC2_INSTANCE_TYPE}" \
# -v "AWS_REGION_NAME=${AWS_REGION_NAME}" $DOCKER_IMAGE_NAME

}

workdflow_build_run_in_docker_container

