# sometimes the indexer gets stuck
# so far only completely resetting the environment fixes it
docker system prune -a -f --volumes
docker system prune -a
docker rm -f (docker ps -a -q)
docker volume rm (docker volume ls -qf dangling=true)

# docker rmi sandbox_algod
# docker rmi sandbox_indexer
# docker rmi postgres

# start (dev mode)
sandbox up dev -v
