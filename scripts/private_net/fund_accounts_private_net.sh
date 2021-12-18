# get a funder account (the genesis accounts change when the network is recreated)
ACCOUNTS_OUTPUT=$(goal account list -d ./net/Node)
echo $ACCOUNTS_OUTPUT
for acct in $(echo "$ACCOUNTS_OUTPUT" | cut -f 3 |tr -s ' '); do
    ACCOUNTS+=($acct)
done
FUNDER=${ACCOUNTS[0]}
echo "Funding account:"
echo $FUNDER

# import additional accounts
goal account import -m "town clutch grain accident sheriff wagon meadow shaft saddle door all town supply indicate deliver about arrange hire kit curve destroy gloom attitude absorb excite"  -d ./net/Node
goal account import -m "phone similar album unusual notable initial evoke party garlic gain west catch bike enforce layer bring suggest shiver script venue couple tooth special abandon ranch"  -d ./net/Node
goal account import -m "abandon include valid approve among begin disorder hint option train palace drink enable enter shallow various bid jacket record left derive memory magnet able phrase"  -d ./net/Node
goal account import -m "clog coral speak since defy siege video lamp polar chronic treat smooth puzzle input payment hobby draft habit race birth ridge correct behave able close"  -d ./net/Node

# fund the accounts
# 10_000 algos
goal clerk send -a 10000000000 -f $FUNDER -t VKCFMGBTVINZ4EN7253QVTALGYQRVMOLVHF6O44O2X7URQP7BAOAXXPFCA -d ./net/Node
goal clerk send -a 10000000000 -f $FUNDER -t WZOKN67NQUMY5ZV7Q2KOBKUY5YP3L5UFFOWBUV6HKXKFMLCUWTNZJRSI4E -d ./net/Node
goal clerk send -a 10000000000 -f $FUNDER -t ZRPA4PEHLXIT4WWEKXFJMWF4FNBCA4P4AYC36H7VGNSINOJXWSQZB2XCP4 -d ./net/Node
goal clerk send -a 10000000000 -f $FUNDER -t MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ -d ./net/Node

# temporary: fund customer payment amount
# to ease manual testing, to not have to send a customer payment first
# note: breaks unit tests
# sandbox goal clerk send -a 10000000000 -f $FUNDER -t 3BW2V2NE7AIFGSARHF7ULZFWJPCOYOJTP3NL6ZQ3TWMSK673HTWTPPKEBA -d ./net/Node

echo "done!"
goal account list -d ./net/Node
