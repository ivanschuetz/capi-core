# get a funder account (the genesis accounts change when the network is recreated)
ACCOUNTS_OUTPUT=$(sandbox goal account list)
echo $ACCOUNTS_OUTPUT
for acct in $(echo "$ACCOUNTS_OUTPUT" | cut -f 3 |tr -s ' '); do
    ACCOUNTS+=($acct)
done
FUNDER=${ACCOUNTS[0]}
echo "Funding account:"
echo $FUNDER

# import additional account
# 7ZLNWP5YP5DCCCLHAYYETZQLFH4GTBEKTBFQDHA723I7BBZ2FKCOZCBE4I
sandbox goal account import -m "group slush snack cram emotion echo cousin viable fan all pupil solar total boss deny under master rely wine help trick mechanic glance abstract clever"

# fund the "funds asset" source - this is an account dedicated solely to mint the funds asset and distribute it to the test accounts
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t DNQPINWK4K5QZYLCK7DVJFEWRUXPXGW36TEUIHNSNOFYI2RMPG2BZPQ7DE

# fund our test accounts
# 10_000 algos
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t STOUDMINSIPP7JMJMGXVJYVS6HHD3TT5UODCDPYGV6KBGP7UYNTLJVJJME
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t 7XSZQUQ2GJB25W37LVM5R4CMKKVC4VNSMIPCIWJYWM5ORA5VA4JRCNOJ4Y
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t PGCS3D5JL4AIFGTBPDGGMMCT3ODKUUFEFG336MJO25CGBG7ORKVOE3AHSU
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t 7ZLNWP5YP5DCCCLHAYYETZQLFH4GTBEKTBFQDHA723I7BBZ2FKCOZCBE4I
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t NIKGABIQLRCPJYCNCFZWR7GUIC3NA66EBVR65JKHKLGLIYQ4KO3YYPV67Q
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t KPV7XSMNSRSQ44QCDQY7I6MORAB4GGT3GRY4WUNTCZZNRKSL4UEBPTJP2U

# multisig accounts
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t DN7MBMCL5JQ3PFUQS7TMX5AH4EEKOBJVDUF4TCV6WERATKFLQF4MQUPZTA
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t GIZTTA56FAJNAN7ACK3T6YG34FH32ETDULBZ6ENC4UV7EEHPXJGGSPCMVU
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t BFRTECKTOOE7A5LHCF3TTEOH2A7BW46IYT2SX5VP6ANKEXHZYJY77SJTVM

# multisig address
sandbox goal clerk send -a 1000000000000 -f $FUNDER -t BSAWQANNI3VWCQH3RCJLDHR27XEYTQYVBLTQ3C2MW5GRULCKFQBEWPDV6E

# temporary: fund customer payment amount
# to ease manual testing, to not have to send a customer payment first
# note: breaks unit tests
# sandbox goal clerk send -a 1000000000000 -f $FUNDER -t 3YZ4RNFDA7XFMN6WLKFFH5BMMQIJQN2OAKRPNTOT5FB3YLRB2HYUCQFDIY

echo "done!"
sandbox goal account list
