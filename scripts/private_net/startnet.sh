# src: https://github.com/algorand/smart-contracts/blob/master/devrel/startnet.sh    
# modified (search for EDIT)
# NOTE also that adding a relay node with relay=true https://github.com/algorand/smart-contracts/blob/3aa355c91e02830d4d7a15449ac1892eee972047/devrel/networktemplate.json#L19-L28,
# for some reason doesn't work with DevMode, so we had to delete it,
# the problem was that it can't start the network (complains about not finding algod.net, which is created as part of starting),
# reson unclear, no verbose mode, and no node.log present, as it's created after starting the network apparently,
# the missing primary relay node is causing this warning during tests: could not make cachedir: mkdir net/Primary/goal.cache: no such file or directory
# it seems to be harmless

#!/bin/bash
set -e
echo “### Creating private network”
goal network create -n tn50e -t networktemplate.json -r net

# EDIT: set some custom settings, before it's started.
sh ./custom_network_settings.sh

echo
echo “### Starting private network” 
goal network start -r net

# EDIT: start kmd - needed in this script to import keys
# we now also rely on kmd to be started after this script
goal kmd start -d net/Node

echo
echo “### Checking node status”
goal network status -r net
echo "### Importing root keys"
NODEKEY=$(goal account list -d net/Node |  awk '{print $2}')
PRIMKEY=$(goal account list -d net/Primary | awk '{print $2}')

echo "Imported ${NODEKEY}"
echo "Imported ${PRIMKEY}"

begin_date=$(date)
s=20
echo ${begin_date}
bd_seconds=$(date '+%s')
echo ${bd_seconds}
num=$(( $bd_seconds + $s ))
echo ${num}
