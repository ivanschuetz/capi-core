export ALGORAND_DATA="/Users/runner/work/make/make/algo_env/net1/data"
export PATH="/Users/runner/work/make/make/algo_env/net1:$PATH"

SRC=$1
echo $SRC
# 10_000 algos
goal clerk send -a 10000000000 -f $SRC -t DN7MBMCL5JQ3PFUQS7TMX5AH4EEKOBJVDUF4TCV6WERATKFLQF4MQUPZTA -d net1/Node
goal clerk send -a 10000000000 -f $SRC -t BFRTECKTOOE7A5LHCF3TTEOH2A7BW46IYT2SX5VP6ANKEXHZYJY77SJTVM -d net1/Node
goal clerk send -a 10000000000 -f $SRC -t GIZTTA56FAJNAN7ACK3T6YG34FH32ETDULBZ6ENC4UV7EEHPXJGGSPCMVU -d net1/Node
goal clerk send -a 10000000000 -f $SRC -t MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ -d net1/Node

# temporary: shares customer payment amount
# note: enabling this currently breaks some Make's unit tests
# goal clerk send -a 10000000000 -f $SRC -t 3BW2V2NE7AIFGSARHF7ULZFWJPCOYOJTP3NL6ZQ3TWMSK673HTWTPPKEBA -d net1/Node
echo "done!"
goal account list -d net1/Node
