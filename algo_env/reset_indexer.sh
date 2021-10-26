#################################
# reset and start indexer
#################################
# note that this can't be executed as part of tests from rust: the background process (the algorand-indexer daemon in the last line) doesn't work when called from vscode (it blocks).
# it also doesn't run as a normal bg process in terminal, apparently the service starts another subprocess or something (doesn't appear under jobs)

# retart the postgres daemon (not sure if needed)
pg_ctl -D /usr/local/var/postgres stop
pg_ctl -D /usr/local/var/postgres start

# stop running indexer instance (there doesn't seem to be anything in the program to do this)
kill -9 $(lsof -ti:8980)

# without this we get ERROR:  DROP DATABASE cannot run inside a transaction block when running sql commands https://stackoverflow.com/a/64937756
# note that postgres here is one of the default databases from postgres
psql -d postgres -c '\set AUTOCOMMIT on'

# create the role
psql -d postgres -c "drop role algorand_test;"
psql -d postgres -c "create role algorand_test login createdb;"

psql -d postgres -c '\set AUTOCOMMIT on'

# re-create the indexer db
psql -d postgres -U algorand_test -c "drop database if exists indexer;"
psql -d postgres -U algorand_test -c "create database indexer;"

# start indexer service
/Users/ischuetz/algorand_tools/algorand-indexer_darwin_amd64_2.6.1/algorand-indexer daemon -P "host=127.0.0.1 port=5432 user=ischuetz dbname=indexer sslmode=disable" --algod=/Users/ischuetz/algo_nets/net1/Node/ 
# note: removed & at the end of previous line: can't use this from rust and in terminal we don't need to run it in the bg

