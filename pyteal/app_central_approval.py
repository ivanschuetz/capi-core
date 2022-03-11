from pyteal import *

"""App central approval"""

tmpl_share_price = Tmpl.Int("TMPL_SHARE_PRICE")
tmpl_capi_app_id = Tmpl.Int("TMPL_CAPI_APP_ID")
tmpl_capi_escrow_address = Tmpl.Addr("TMPL_CAPI_ESCROW_ADDRESS")
tmpl_precision = Tmpl.Int("TMPL_PRECISION__") # note "__" at the end so text isn't contained in TMPL_PRECISION_SQUARE
tmpl_capi_share = Tmpl.Int("TMPL_CAPI_SHARE")
tmpl_precision_square = Tmpl.Int("TMPL_PRECISION_SQUARE")
tmpl_investors_share = Tmpl.Int("TMPL_INVESTORS_SHARE")
tmpl_share_supply = Tmpl.Int("TMPL_SHARE_SUPPLY")

GLOBAL_RECEIVED_TOTAL = "CentralReceivedTotal"
GLOBAL_CENTRAL_ESCROW_ADDRESS = "CentralEscrowAddress"
GLOBAL_CUSTOMER_ESCROW_ADDRESS = "CustomerEscrowAddress"
GLOBAL_SHARES_ASSET_ID = "SharesAssetId"
GLOBAL_FUNDS_ASSET_ID = "FundsAssetId"
LOCAL_SHARES = "Shares"
LOCAL_HARVESTED_TOTAL = "HarvestedTotal"
LOCAL_PROJECT_ID = "Project"

def approval_program():
    is_create = Gtxn[0].application_id() == Int(0)
    handle_create = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall), 
        Approve()
    )

    is_setup_dao = Global.group_size() == Int(10)
    handle_setup_dao = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_args.length() == Int(4)),
        Assert(Gtxn[1].type_enum() == TxnType.Payment),
        Assert(Gtxn[1].receiver() == Gtxn[0].application_args[0]),
        Assert(Gtxn[2].type_enum() == TxnType.Payment),
        Assert(Gtxn[2].receiver() == Gtxn[0].application_args[1]),
        Assert(Gtxn[3].type_enum() == TxnType.Payment),
        Assert(Gtxn[4].type_enum() == TxnType.Payment),
        Assert(Gtxn[5].type_enum() == TxnType.AssetTransfer), # optin locking escrow to shares
        Assert(Gtxn[5].asset_amount() == Int(0)),
        Assert(Gtxn[6].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[6].asset_amount() == Int(0)),
        Assert(Gtxn[7].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[7].asset_amount() == Int(0)),
        Assert(Gtxn[8].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[8].asset_amount() == Int(0)),
        Assert(Gtxn[9].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[9].xfer_asset() == Btoi(Gtxn[0].application_args[2])),
        App.globalPut(Bytes(GLOBAL_RECEIVED_TOTAL), Int(0)),
        App.globalPut(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS), Gtxn[0].application_args[0]),
        App.globalPut(Bytes(GLOBAL_CUSTOMER_ESCROW_ADDRESS), Gtxn[0].application_args[1]),
        App.globalPut(Bytes(GLOBAL_SHARES_ASSET_ID), Btoi(Gtxn[0].application_args[2])),
        App.globalPut(Bytes(GLOBAL_FUNDS_ASSET_ID), Btoi(Gtxn[0].application_args[3])),
        Approve()
    )

    is_optin = Global.group_size() == Int(1)
    handle_optin = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.OptIn),
        Approve()
    )

    is_unlock = And(
        Gtxn[0].type_enum() == TxnType.ApplicationCall,
        Gtxn[0].on_completion() == OnComplete.CloseOut,
        Gtxn[1].type_enum() == TxnType.AssetTransfer,
    )
    handle_unlock = Seq(
        Assert(Gtxn[1].asset_amount() > Int(0)), # unlocking > 0 shares TODO similar checks in other contracts
        Assert(Gtxn[1].asset_receiver() == Gtxn[0].sender()), # shares receiver is the app caller
        Assert(Gtxn[1].asset_amount() == App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES))), # shares xfer == owned shares count
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))), # shares is the DAO shares asset
        Approve()
    )
 
    total_entitled_harvest_amount = Div(
        Mul(
            Div(
                Mul(
                    Mul(App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES)), tmpl_precision), 
                    tmpl_investors_share
                ), 
                # Mul(App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES)), tmpl_share_supply), 
                tmpl_share_supply
            ), 
            App.globalGet(Bytes(GLOBAL_RECEIVED_TOTAL))
        ), 
        tmpl_precision_square
    )

    # Expect harvester to be the gtxn 0 sender. Calculates entitled harvest based on LOCAL_SHARES and LOCAL_HARVESTED_TOTAL.
    entitled_harvest_amount = Minus(total_entitled_harvest_amount, App.localGet(Gtxn[0].sender(), Bytes(LOCAL_HARVESTED_TOTAL)))
    wants_to_harvest_less_or_eq_to_entitled_amount = Ge(entitled_harvest_amount, Gtxn[1].asset_amount())

    is_harvest = Gtxn[1].sender() == App.globalGet(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS))
    handle_harvest = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))), # the harvested asset is the funds asset 
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 
        Assert(wants_to_harvest_less_or_eq_to_entitled_amount),
        App.localPut(
            Gtxn[0].sender(), 
            Bytes(LOCAL_HARVESTED_TOTAL), 
            Add(
                App.localGet(Gtxn[0].sender(), Bytes(LOCAL_HARVESTED_TOTAL)), 
                Gtxn[1].asset_amount()
            )
        ),
        Approve()
    )

    # expects the 2 first txs of invest / lock to be the app call and lock (shares transfer to locking escrow)
    lock_shares = Seq(
        Assert(Gtxn[1].asset_amount() > Int(0)), # sanity: don't allow locking 0 shares 
        App.localPut( # set / increment share count in local state
            Gtxn[0].sender(), 
            Bytes(LOCAL_SHARES), 
            Add(
                App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES)),
                Gtxn[1].asset_amount()
            )
        ),
        App.localPut( # initialize already harvested local state
            Gtxn[0].sender(), 
            Bytes(LOCAL_HARVESTED_TOTAL), 
            # NOTE that this sets HarvestedTotal to the entitled amount each time that the investor buys/locks shares
            # meaning that investors may lose pending dividend by buying or locking new shares
            # TODO improve? - a non TEAL way could be to just automatically retrieve pending dividend in the same group 
            # see more notes in old repo
            entitled_harvest_amount
            # Gtxn[1].asset_amount()
        ),
    )

    # For invest/lock. Project id expected as first arg of the first tx
    # save the project id in local state, so we can find projects where a user invested in (with the indexer)  
    # TODO rename in CapiProject or similar - this key is used to filter for txs belonging to capi / project id use case
    # - we don't have the app id when querying this, only the sender account and this key
    save_project_id = App.localPut(Gtxn[0].sender(), Bytes(LOCAL_PROJECT_ID), Gtxn[0].application_args[0])

    is_lock = Global.group_size() == Int(2)
    handle_lock = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_args.length() == Int(1)),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # app caller is locking the shares
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))), # the locked assets have the DAO's share asset id

        lock_shares,
        save_project_id,
        
        Approve()
    )

    is_drain = And(
        Global.group_size() == Int(4), 
        Gtxn[2].sender() == App.globalGet(Bytes(GLOBAL_CUSTOMER_ESCROW_ADDRESS))
    )

    drain_asset_balance = AssetHolding.balance(Gtxn[2].sender(), Gtxn[2].xfer_asset())

    handle_drain = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall), # dao app call
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[1].type_enum() == TxnType.ApplicationCall), # capi app call
        Assert(Gtxn[1].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer), # drain
        Assert(Gtxn[2].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))), # draining the funds asset
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer), # capi share
        Assert(Gtxn[3].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))), # capi share is also in funds asset
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # same user is calling both apps
        Assert(Gtxn[2].asset_receiver() == App.globalGet(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS))), # the funds are being drained to the central escrow
        Assert(Gtxn[3].asset_receiver() == tmpl_capi_escrow_address), # the capi fee is being sent to the capi escrow

        drain_asset_balance, # needs to be listed like this, see: https://forum.algorand.org/t/using-global-get-ex-on-noop-call-giving-error-when-deploying-app/5314/2?u=user123
        AssetHolding.balance(Gtxn[2].sender(), Gtxn[2].xfer_asset()),
        Assert(
            Gtxn[3].asset_amount() == Div(
                Mul(
                    Mul(drain_asset_balance.value(), tmpl_precision),
                    tmpl_capi_share
                ),
                tmpl_precision_square
            )
        ),

        # TODO - store capi escrow in global state (see orig TEAL)
        App.globalPut(
            Bytes(GLOBAL_RECEIVED_TOTAL), 
            Add(App.globalGet(Bytes(GLOBAL_RECEIVED_TOTAL)), Gtxn[2].asset_amount())
        ),
        Approve()
    )
    
    is_invest = And(
        Global.group_size() == Int(4), 
    )
    handle_invest = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall), # app call
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_args.length() == Int(1)),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer), # shares xfer
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))), # receiving shares asset
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer), # pay for shares (in funds asset)
        Assert(Gtxn[2].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))), # paying with funds asset
        Assert(Gtxn[2].asset_receiver() == App.globalGet(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS))), # paying to the central escrow
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer), # optin to shares
        Assert(Gtxn[3].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))),
        Assert(Gtxn[3].asset_amount() == Int(0)),
        Assert(Gtxn[3].asset_receiver() == Gtxn[3].sender()), # TODO is this optin check needed - if yes add it to other optins
        # the investor sends 3 txs (app call, pay for shares, shares optin)
        Assert(Gtxn[0].sender() == Gtxn[2].sender()),
        Assert(Gtxn[2].sender() == Gtxn[3].sender()),

        # TODO check that gtxn 1 sender is invest escrow and receiver locking escrow? (add both to state)
        Assert(Gtxn[2].asset_amount() == Mul(Gtxn[1].asset_amount(), tmpl_share_price)), # Paying the correct price for the bought shares

        lock_shares,
        save_project_id,

        Approve()
    )

################################################
# TODO the branching here is a bit weird - modelled (mostly) after original TEAL
# can this be improved - we use group size and arguments to identify the use cases,
# so we've to branch based on group size / args length
################################################
    is_group_size2 = Global.group_size() == Int(2)
    handle_group_size2 = Cond(
        [is_unlock, handle_unlock],
        [is_harvest, handle_harvest],
        [is_lock, handle_lock], # TODO jump directly here without condition check (it's like "default" clause if group size is 2, which we already know)
    )
    
    is_group_size4 = Global.group_size() == Int(4)
    handle_group_size4 = Cond(
        [is_drain, handle_drain],
        [is_invest, handle_invest],
    )
################################################
   
    program = Cond(
        [is_create, handle_create],
        [is_setup_dao, handle_setup_dao],
        [is_optin, handle_optin],
        [is_group_size2, handle_group_size2],
        [is_group_size4, handle_group_size4],
    )

    return compileTeal(program, Mode.Application, version=5)

def clear_program():
    return compileTeal(Int(1), Mode.Application, version=5)
 
path = 'teal_template/app_central_approval.teal'
with open(path, 'w') as f:
    output = approval_program()
    # print(output)
    f.write(output)
    print("Done! output: " + path)

def export(path, output):
   with open(path, "w") as f:
    # print(output)
    f.write(output)
    print("Wrote TEAL to: " + path)

export("teal_template/app_central_approval.teal", approval_program())
export("teal/app_central_clear.teal", clear_program())

print("Done! Wrote central approval and clear TEAL")
