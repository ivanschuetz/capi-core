# https://developer.algorand.org/docs/run-a-node/setup/install/#installing-on-a-mac

cd /Users/runner/work/make/make/algo_env

mkdir ./net1
cd ./net1
curl https://raw.githubusercontent.com/algorand/go-algorand-doc/master/downloads/installers/update.sh -O
chmod 544 update.sh
./update.sh -i -c stable -p . -d ./data -n

mkdir ~/node
cd ~/node
curl https://raw.githubusercontent.com/algorand/go-algorand-doc/master/downloads/installers/update.sh -O
chmod 544 update.sh
./update.sh -i -c stable -p ~/node -d ~/node/data -n

echo "$HOME:"
echo $HOME
echo "pwd:"
pwd

# export ALGORAND_DATA="/Users/runner/work/make/make/algo_env/net1/data"
# export PATH="/Users/runner/work/make/make/algo_env/net1:$PATH"

export ALGORAND_DATA="$HOME/node/data"
export PATH="$HOME/node:$PATH"

echo "path:"
echo $PATH

echo "which goal:"
which goal

algod -v

