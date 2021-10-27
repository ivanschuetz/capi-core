curl -L https://github.com/docker/compose/releases/download/1.24.1/docker-compose-`uname -s`-`uname -m` -o /usr/local/bin/docker-compose
chmod +x /usr/local/bin/docker-compose

docker-compose --version

git clone https://github.com/algorand/sandbox.git
cd sandbox
./sandbox up

# # https://developer.algorand.org/docs/run-a-node/setup/install/#installing-with-debian-packages-debian-ubuntu
# 
# sudo apt-get update
# sudo apt-get install -y gnupg2 curl software-properties-common
# curl -O https://releases.algorand.com/key.pub
# sudo apt-key add key.pub
# sudo add-apt-repository "deb [arch=amd64] https://releases.algorand.com/deb/ stable main"
# sudo apt-get update
# 
# # To get both algorand and the devtools:
# # sudo apt-get install -y algorand-devtools
# 
# # Or, to only install algorand:
# sudo apt-get install -y algorand
# 
# algod -v

