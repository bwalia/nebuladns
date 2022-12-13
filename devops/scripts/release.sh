#!/bin/bash

# ############ This bash script deploys odincm.com into the kubernetes.
# set -x
# if [[ -z "$1" ]]; then
#    echo "env is empty, so setting targetEnv to development (default)"
#    targetEnv="dev"
# else
#    echo "env is provided, so setting targetEnv to $1"
#    targetEnv=$1
# fi

# if [[ -z "$2" ]]; then
#    echo "action is empty, so setting action to install (default)"
#    actionStr="build_install"
# else
#    echo "action is empty, so setting action to $2"
#    actionStr=$2
# fi

# if [[ "$actionStr" == "build_install" ]]; then
# docker build -f devops/docker/Dockerfile -t bwalia/odincm . --no-cache
# docker tag bwalia/odincm registry.workstation.co.uk/odincm:$targetEnv
# docker push registry.workstation.co.uk/odincm:$targetEnv
# fi

# # kubectl set image deployment/odincm odincm=registry.workstation.co.uk/odincm:$targetEnv
# helm uninstall odincm-${targetEnv} -n ${targetEnv}
# helm upgrade --install -f devops/odincm/values-${targetEnv}.yaml odincm-${targetEnv} ./devops/odincm-chart --set-string targetImage="registry.workstation.co.uk/odincm" --set-string targetImageTag="${targetEnv}" --namespace ${targetEnv}

targetEnv=test
IMAGE_TAG=dev


helm uninstall odincm-${targetEnv} -n ${targetEnv}
helm upgrade --install -f devops/odincm-chart/values-${targetEnv}-k3s2.yaml odincm-${targetEnv} devops/odincm-chart --set-string targetImage="registry.workstation.co.uk/odincm-node" --set-string targetImageTag="${IMAGE_TAG}" --namespace ${targetEnv}