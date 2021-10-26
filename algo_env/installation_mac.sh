# https://developer.algorand.org/docs/run-a-node/setup/install/#installing-on-a-mac

mkdir ~/node
cd ~/node
curl https://raw.githubusercontent.com/algorand/go-algorand-doc/master/downloads/installers/update.sh -O
chmod 544 update.sh
./update.sh -i -c stable -p ~/node -d ~/node/data -n

export ALGORAND_DATA="$HOME/node/data"
export PATH="$HOME/node:$PATH"

algod -v

