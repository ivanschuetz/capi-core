# sometimes the indexer gets stuck
# so far only completely resetting the environment fixes it
docker system prune -a -f --volumes
docker system prune -a
docker rm -f (docker ps -a -q)
docker volume rm (docker volume ls -qf dangling=true)

# start (dev mode)
sandbox up dev -v
