goal network stop -r ./net1
goal kmd stop -d ./net1/Node
rm -r ./net1
sh startnet.sh

# use always the same algod port
# note: not needed for kmd, which uses a fixed port by default 
cp -f ./config.json ./net1/Node

# use always the same tokens, to not have to set environment variables again
echo "44d70009a00561fe340b2584a9f2adc6fec6a16322554d44f56bef9e682844b9" > net1/Node/algod.token
echo "44d70009a00561fe340b2584a9f2adc6fec6a16322554d44f56bef9e682844b9" > net1/Node/kmd-v0.5/kmd.token

goal network start -r ./net1

# seems to need a restart to load the correct host:port from the Node config
goal network stop -r ./net1
goal network start -r ./net1

goal kmd start -d ./net1/Node

# displays Rust code with algod / kmd settings
# commented: getting this from env now
# ./output_connection

# import accounts
# GIZTTA56FAJNAN7ACK3T6YG34FH32ETDULBZ6ENC4UV7EEHPXJGGSPCMVU
goal account import -m "fire enlist diesel stamp nuclear chunk student stumble call snow flock brush example slab guide choice option recall south kangaroo hundred matrix school above zero" -d net1/Node
# BFRTECKTOOE7A5LHCF3TTEOH2A7BW46IYT2SX5VP6ANKEXHZYJY77SJTVM
goal account import -m "since during average anxiety protect cherry club long lawsuit loan expand embark forum theory winter park twenty ball kangaroo cram burst board host ability left" -d net1/Node
# DN7MBMCL5JQ3PFUQS7TMX5AH4EEKOBJVDUF4TCV6WERATKFLQF4MQUPZTA
goal account import -m "auction inquiry lava second expand liberty glass involve ginger illness length room item discover ahead table doctor term tackle cement bonus profit right above catch" -d net1/Node
# MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ
goal account import -m "clog coral speak since defy siege video lamp polar chronic treat smooth puzzle input payment hobby draft habit race birth ridge correct behave able close" -d net1/Node

# army, royal, inherit, gold, remain, alley, type, worry, melody, random, rebel, lumber, taste, exit, always, beauty, snap, tragic, panther, tragic, fit, abuse, insane, absorb, brand

# show accounts
goal account list -d ./net1/Node

