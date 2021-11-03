# run before each test

# ensure sandbox is up
sandbox up dev

# reset sandbox
sandbox reset

# import additional account
# MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ
sandbox goal account import -m "clog coral speak since defy siege video lamp polar chronic treat smooth puzzle input payment hobby draft habit race birth ridge correct behave able close"

# fund additional account

FUNDER=VKCFMGBTVINZ4EN7253QVTALGYQRVMOLVHF6O44O2X7URQP7BAOAXXPFCA
# echo $FUNDER
# 10_000 algos
sandbox goal clerk send -a 10000000000 -f $FUNDER -t MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ

# temporary: fund customer payment amount
# to ease manual testing, to not have to send a customer payment first
# note: breaks unit tests
# sandbox goal clerk send -a 10000000000 -f $FUNDER -t 3BW2V2NE7AIFGSARHF7ULZFWJPCOYOJTP3NL6ZQ3TWMSK673HTWTPPKEBA -d net1/Node

echo "done!"
sandbox goal account list
