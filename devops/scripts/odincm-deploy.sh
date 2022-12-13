# Run docker compose

NODE_VERSION="12.22.12-alpine3.15"
TARGET_CLUSTER="k3s0"
TARGET_CLUSTER="k3s2"
TARGET_STACK="node"
DATE_GEN_VERSION=$(date +"%Y%m%d%I%M%S")
DATE_GEN_VERSION="dev"
echo "The node version base image: $NODE_VERSION"

if [[ -z "$1" ]]; then
   echo "env is empty, so setting targetEnv to development (default)"
   targetEnv="dev"
else
   echo "env is provided, so setting targetEnv to $1"
   targetEnv=$1
fi

if [[ -z "$2" ]]; then
   echo "namespace is empty, so setting namespace to dev (default)"
   targetNs="dev"
else
   echo "namespace is provided, so setting namespace to $2"
   targetNs=$2
fi

if [[ -z "$3" ]]; then
   echo "action is empty, so setting action to install (default)"
   cicd_action="install"
else
   echo "action is provided, action is set to $3"
   cicd_action=$3
fi

if [[ -z "$4" ]]; then
   echo "k3s deployment tool type is empty, so setting k3s_deployment_tool to helm (default)"
   k3s_deployment_tool="helm"
else
   echo "k3s deployment tool type is provided, k3s_deployment_tool is set to $4"
   k3s_deployment_tool=$4
fi

if [[ "$targetEnv" == "dev" ]]; then
echo "No need to load kubeconfig use default var KUBE_CONFIG"
else
if [[ -z "$5" ]]; then
echo "KUBECONFIG is empty, so leaving default set KUBECONFIG to whatever it may be (default)"
else
echo "KUBECONFIG is provided, so setting KUBECONFIG $5"
export KUBECONFIG=$5
fi
fi

if [[ -z "$6" ]]; then
echo "VIRTUAL_HOST is empty, so leaving default set VIRTUAL_HOST to whatever it may be (default ${SVC_HOST})"
export VIRTUAL_HOST=${SVC_HOST}
else
echo "VIRTUAL_HOST is provided, so setting VIRTUAL_HOST $6"
export VIRTUAL_HOST=$6
fi

if [[ -z "$7" ]]; then  # docker_base_image not in use yet
   echo "docker base image is empty, so setting docker base image to dev-odincm-webserver (default)"
   docker_base_image="${targetEnv}-odincm"
else
   echo "docker base image type is provided, docker base image is set to $7"
   docker_base_image=$7
fi

if [[ "$targetEnv" == "dev" ]]; then
echo "No need to move env files in case local dev env"
else
cp ${WORKSPACE_DIR}/${targetEnv}.env ${WORKSPACE_DIR}/.env
fi

if [[ -z "$8" ]]; then
echo "TARGET_CLUSTER is default, so leaving default set TARGET_CLUSTER to whatever it may be (default ${TARGET_CLUSTER})"
export TARGET_CLUSTER=${TARGET_CLUSTER}
else
echo "TARGET_CLUSTER is provided, so setting TARGET_CLUSTER $8"
export TARGET_CLUSTER=$8
fi

if [[ -z "$9" ]]; then
echo "Docker build cmd is default, so leaving default set BUILD_IMAGE_APP to whatever it may be (nerdctl)"
export BUILD_IMAGE_APP="nerdctl"
else
echo "BUILD_IMAGE_APP is provided, so setting BUILD_IMAGE_APP $9"
export BUILD_IMAGE_APP=$9
fi

if [[ -z "$10" ]]; then
   echo "The node version (lts)"
   NODE_VERSION="12.22.12-alpine3.15"
else
   echo "The node version set to $10"
   NODE_VERSION=$10
fi

VALUES_FILE_PATH=values-${targetNs}-${TARGET_CLUSTER}.yaml

#${BUILD_IMAGE_APP} build -f $(pwd)/devops/docker/Dockerfile-prod-yarn --build-arg NODE_VERSION=$NODE_VERSION -t odincm-${TARGET_STACK} . --no-cache
${BUILD_IMAGE_APP} build -f $(pwd)/devops/docker/Dockerfile-nuxt-server -t odincm-${TARGET_STACK} . --no-cache
${BUILD_IMAGE_APP} tag odincm-${TARGET_STACK} registry.workstation.co.uk/odincm-${TARGET_STACK}:${DATE_GEN_VERSION}
${BUILD_IMAGE_APP} push registry.workstation.co.uk/odincm-${TARGET_STACK}:${DATE_GEN_VERSION}

helm upgrade --install -f devops/odincm-chart/${VALUES_FILE_PATH} odincm-${targetNs} ./devops/odincm-chart --set-string targetImage="registry.workstation.co.uk/odincm-${TARGET_STACK}" --set-string targetImageTag="${DATE_GEN_VERSION}" --namespace ${targetNs} --create-namespace

# ${BUILD_IMAGE_APP} container stop odincm
# ${BUILD_IMAGE_APP} container rm odincm

# ${BUILD_IMAGE_APP} run --name odincm -p 3011:3000 -d edgeone/odincm
