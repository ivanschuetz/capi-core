export ALGORAND_DATA="$HOME/node/data"
export PATH="$HOME/node:$PATH"

#!/bin/bash
set -e
goal network stop -r test
goal network delete -r test
