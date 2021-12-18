# custom network settings
# - EndpointAddress: to use always the same algod port (not needed for kmd, which uses a fixed port by default)
# - EnableDeveloperAPI: e.g. enables TEAL compilation
cp -f ./config.json ./net1/Node

# use always the same tokens, to not have to set environment variables again
echo "44d70009a00561fe340b2584a9f2adc6fec6a16322554d44f56bef9e682844b9" > net1/Node/algod.token
echo "44d70009a00561fe340b2584a9f2adc6fec6a16322554d44f56bef9e682844b9" > net1/Node/kmd-v0.5/kmd.token
