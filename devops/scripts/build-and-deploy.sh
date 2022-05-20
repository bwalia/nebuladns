# Run docker compose

docker build -f devops/docker/Dockerfile -t edgeone/odincm .

docker container stop odincm
docker container rm odincm

docker run --name odincm -p 3011:3000 -d edgeone/odincm
