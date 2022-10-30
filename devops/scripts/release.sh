#!/bin/bash
############ This bash script deploys odincm.com into the kubernetes.
set -x
if [[ -z "$1" ]]; then
   echo "env is empty, so setting targetEnv to development (default)"
   targetEnv="dev"
else
   echo "env is provided, so setting targetEnv to $1"
   targetEnv=$1
fi

docker build -f devops/docker/Dockerfile -t bwalia/odincm .     #--no-cache
docker tag bwalia/odincm registry.workstation.co.uk/odincm:$targetEnv
docker push registry.workstation.co.uk/odincm:$targetEnv

# kubectl set image deployment/odincm odincm=registry.workstation.co.uk/odincm:$targetEnv

helm upgrade --install -f devops/odincm-chart/values-${targetEnv}.yaml odincm-${targetEnv} ./devops/odincm-chart --set-string targetImage="registry.workstation.co.uk/odincm" --set-string targetImageTag="${targetEnv}" --namespace ${targetEnv}
