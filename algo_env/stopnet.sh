export ALGORAND_DATA="/Users/runner/work/make/make/algo_env/net1/data"
export PATH="/Users/runner/work/make/make/algo_env/net1:$PATH"

#!/bin/bash
set -e
goal network stop -r test
goal network delete -r test
