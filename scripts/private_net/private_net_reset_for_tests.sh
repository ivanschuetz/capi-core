# run before each test
# the private network is created under ./net

sh ./stop_and_delete_private_net.sh
sh ./startnet.sh
sh ./fund_accounts_private_net.sh
