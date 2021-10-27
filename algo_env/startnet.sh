export ALGORAND_DATA="$HOME/node/data"
export PATH="$HOME/node:$PATH"

cd /Users/runner/work/make/make/algo_env

#!/bin/bash
set -e
echo “### Creating private network”
goal network create -n tn50e -t networktemplate.json -r ./net1
echo

echo "/////////////// after creating private network, net1 contents:"
ls ./net1
echo "/////////////// after creating private network, primary node contents:"
ls ./net1/Primary
echo "/////////////// after creating private network, node contents:"
ls ./net1/Node

echo “### Starting private network”
# goal network start -r ./net1
goal network start -r ./net1 -d ./net1/Node
echo
echo “### Checking node status”
goal network status -r ./net1
echo "### Importing root keys"
NODEKEY=$(goal account list -d net1/Node |  awk '{print $2}')
PRIMKEY=$(goal account list -d net1/Primary | awk '{print $2}')

echo "Imported ${NODEKEY}"
echo "Imported ${PRIMKEY}"

begin_date=$(date)
s=20
echo ${begin_date}
bd_seconds=$(date '+%s')
echo ${bd_seconds}
num=$(( $bd_seconds + $s ))
echo ${num}

echo "/////////////// startnet.sh finished creating private network. net1 contents:"
ls /Users/runner/work/make/make/algo_env/net1
echo "///////////////"

