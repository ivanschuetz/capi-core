# run before each test

# ensure ../sandbox/sandbox is up
# ../sandbox/sandbox down
../sandbox/sandbox up dev -v

# reset
../sandbox/sandbox reset

echo "!!!!!"
../sandbox/sandbox goal account list
echo "!!!!!"

# get a funder account (the sandbox accounts are variable - change when the sandbox environment is recreated)
ACCOUNTS_OUTPUT=$(../sandbox/sandbox goal account list)
echo $ACCOUNTS_OUTPUT
for acct in $(echo "$ACCOUNTS_OUTPUT" | cut -f 3 |tr -s ' '); do
    ACCOUNTS+=($acct)
done
FUNDER=${ACCOUNTS[0]}
echo "Funding account:"
echo $FUNDER

# import additional account
# MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ
../sandbox/sandbox goal account import -m "clog coral speak since defy siege video lamp polar chronic treat smooth puzzle input payment hobby draft habit race birth ridge correct behave able close"

# fund our test accounts
../sandbox/sandbox goal clerk send -a 10000000000 -f $FUNDER -t VKCFMGBTVINZ4EN7253QVTALGYQRVMOLVHF6O44O2X7URQP7BAOAXXPFCA
../sandbox/sandbox goal clerk send -a 10000000000 -f $FUNDER -t WZOKN67NQUMY5ZV7Q2KOBKUY5YP3L5UFFOWBUV6HKXKFMLCUWTNZJRSI4E
../sandbox/sandbox goal clerk send -a 10000000000 -f $FUNDER -t ZRPA4PEHLXIT4WWEKXFJMWF4FNBCA4P4AYC36H7VGNSINOJXWSQZB2XCP4
../sandbox/sandbox goal clerk send -a 10000000000 -f $FUNDER -t MKRBTLNZRS3UZZDS5OWPLP7YPHUDNKXFUFN5PNCJ3P2XRG74HNOGY6XOYQ

# temporary: fund customer payment amount
# to ease manual testing, to not have to send a customer payment first
# note: breaks unit tests
# sandbox goal clerk send -a 10000000000 -f $FUNDER -t 3BW2V2NE7AIFGSARHF7ULZFWJPCOYOJTP3NL6ZQ3TWMSK673HTWTPPKEBA -d net1/Node

echo "done!"
../sandbox/sandbox goal account list
