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
LOCAL_CLAIMED_TOTAL = "ClaimedTotal"
LOCAL_DAO_ID = "Dao"

def approval_program():
    handle_create = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall), 
        Approve()
    )

    handle_setup_dao = Seq(
        # app call
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_args.length() == Int(4)),

        # creator sends min balance to central escrow
        Assert(Gtxn[1].type_enum() == TxnType.Payment),
        Assert(Gtxn[1].receiver() == Gtxn[0].application_args[0]),

        # creator sends min balance to customer escrow
        Assert(Gtxn[2].type_enum() == TxnType.Payment),
        Assert(Gtxn[2].receiver() == Gtxn[0].application_args[1]),

        # creator sends min balance to locking escrow
        Assert(Gtxn[3].type_enum() == TxnType.Payment),

        # creator sends min balance to investing escrow
        Assert(Gtxn[4].type_enum() == TxnType.Payment),

        # locking escrow opt-ins to shares
        Assert(Gtxn[5].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[5].asset_amount() == Int(0)),

        # investing escrow opt-ins to shares
        Assert(Gtxn[6].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[6].asset_amount() == Int(0)),

        # central escrow opt-ins to funds asset
        Assert(Gtxn[7].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[7].asset_amount() == Int(0)),

        # customer escrow opt-ins to funds asset
        Assert(Gtxn[8].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[8].asset_amount() == Int(0)),

        # creator transfers shares to investing escrow
        Assert(Gtxn[9].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[9].xfer_asset() == Btoi(Gtxn[0].application_args[2])),

        # initialize state
        App.globalPut(Bytes(GLOBAL_RECEIVED_TOTAL), Int(0)),
        App.globalPut(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS), Gtxn[0].application_args[0]),
        App.globalPut(Bytes(GLOBAL_CUSTOMER_ESCROW_ADDRESS), Gtxn[0].application_args[1]),
        App.globalPut(Bytes(GLOBAL_SHARES_ASSET_ID), Btoi(Gtxn[0].application_args[2])),
        App.globalPut(Bytes(GLOBAL_FUNDS_ASSET_ID), Btoi(Gtxn[0].application_args[3])),

        Approve()
    )

    handle_optin = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),
        Assert(Gtxn[0].on_completion() == OnComplete.OptIn),
        Approve()
    )

    handle_unlock = Seq(
        Assert(Global.group_size() == Int(2)),

        # app call to opt-out
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.CloseOut),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),

        # shares xfer to the investor
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].asset_receiver() == Gtxn[0].sender()), # shares receiver is the app caller
        Assert(Gtxn[1].asset_amount() == App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES))), # shares xfer == owned shares count
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))),

        Approve()
    )
 
    total_entitled_dividend = Div(
        Mul(
            Div(
                Mul(
                    Mul(App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES)), tmpl_precision), 
                    tmpl_investors_share
                ), 
                tmpl_share_supply
            ), 
            App.globalGet(Bytes(GLOBAL_RECEIVED_TOTAL))
        ), 
        tmpl_precision_square
    )

    # Calculates claimable dividend based on LOCAL_SHARES and LOCAL_CLAIMED_TOTAL.
    # Expects claimer to be the gtxn 0 sender. 
    claimable_dividend = Minus(total_entitled_dividend, App.localGet(Gtxn[0].sender(), Bytes(LOCAL_CLAIMED_TOTAL)))
    wants_to_claim_less_or_eq_than_claimable_dividend = Ge(claimable_dividend, Gtxn[1].asset_amount())

    # note that identification is different between app and central_escrow - needed? TODO review
    handle_claim = Seq(
        Assert(Global.group_size() == Int(2)),

        # app call to verify and set dividend
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 
        
        # xfer to transfer dividend to investor
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].sender() == App.globalGet(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS))),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))), # the claimed asset is the funds asset 

        # verify dividend amount is correct
        Assert(wants_to_claim_less_or_eq_than_claimable_dividend),
        
        # update local state with retrieved dividend
        App.localPut(
            Gtxn[0].sender(), 
            Bytes(LOCAL_CLAIMED_TOTAL), 
            Add(
                App.localGet(Gtxn[0].sender(), Bytes(LOCAL_CLAIMED_TOTAL)), 
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
        App.localPut( # initialize already claimed local state
            Gtxn[0].sender(), 
            Bytes(LOCAL_CLAIMED_TOTAL), 
            # NOTE that this sets claimedTotal to the entitled amount each time that the investor buys/locks shares
            # meaning that investors may lose pending dividend by buying or locking new shares
            # TODO improve? - a non TEAL way could be to just automatically retrieve pending dividend in the same group 
            # see more notes in old repo
            claimable_dividend
            # Gtxn[1].asset_amount()
        ),
    )

    # For invest/lock. Dao id expected as first arg of the first tx
    # save the dao id in local state, so we can find daos where a user invested in (with the indexer)  
    # TODO rename in CapiDao or similar - this key is used to filter for txs belonging to capi / dao id use case
    # - we don't have the app id when querying this, only the sender account and this key
    save_dao_id = App.localPut(Gtxn[0].sender(), Bytes(LOCAL_DAO_ID), Gtxn[0].application_args[1])

    handle_lock = Seq(
        Assert(Global.group_size() == Int(2)),

        # app call to update state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_args.length() == Int(2)),
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # app caller is locking the shares

        # send shares to locking escrow
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))),

        # save shares on local state
        lock_shares,

        # save the dao id on local state
        save_dao_id,

        Approve()
    )

    drain_asset_balance = AssetHolding.balance(Gtxn[2].sender(), Gtxn[2].xfer_asset())

    handle_drain = Seq(
        Assert(Global.group_size() == Int(4)), 

        # call app to verify amount and update state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # same user is calling both apps

        # call capi app to update state
        Assert(Gtxn[1].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[1].application_id() == tmpl_capi_app_id),
        Assert(Gtxn[1].on_completion() == OnComplete.NoOp),

        # drain: funds xfer to central escrow
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].asset_amount() > Int(0)),
        Assert(Gtxn[2].sender() == App.globalGet(Bytes(GLOBAL_CUSTOMER_ESCROW_ADDRESS))),
        Assert(Gtxn[2].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))),
        Assert(Gtxn[2].asset_receiver() == App.globalGet(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS))),

        # pay capi fee: funds xfer to capi escrow
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[3].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))),
        Assert(Gtxn[3].asset_receiver() == tmpl_capi_escrow_address),

        # check that capi fee is correct
        drain_asset_balance, # needs to be listed like this, see: https://forum.algorand.org/t/using-global-get-ex-on-noop-call-giving-error-when-deploying-app/5314/2?u=user123
        # AssetHolding.balance(Gtxn[2].sender(), Gtxn[2].xfer_asset()),
        Assert(
            Gtxn[3].asset_amount() == Div(
                Mul(
                    Mul(drain_asset_balance.value(), tmpl_precision),
                    tmpl_capi_share
                ),
                tmpl_precision_square
            )
        ),

        # update total received
        App.globalPut(
            Bytes(GLOBAL_RECEIVED_TOTAL), 
            Add(App.globalGet(Bytes(GLOBAL_RECEIVED_TOTAL)), Gtxn[2].asset_amount())
        ),

        Approve()
    )
    
    handle_invest = Seq(
        Assert(Global.group_size() == Int(4)),

        # app call to initialize shares state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == Global.current_application_id()),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_args.length() == Int(2)),

        # shares xfer to investor
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))), # receiving shares asset

        # investor pays for shares: funds xfer to central escrow
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].asset_amount() > Int(0)),
        Assert(Gtxn[2].xfer_asset() == App.globalGet(Bytes(GLOBAL_FUNDS_ASSET_ID))),
        Assert(Gtxn[2].asset_receiver() == App.globalGet(Bytes(GLOBAL_CENTRAL_ESCROW_ADDRESS))),

        # investor opts-in to shares 
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer), # optin to shares
        Assert(Gtxn[3].xfer_asset() == App.globalGet(Bytes(GLOBAL_SHARES_ASSET_ID))),
        Assert(Gtxn[3].asset_amount() == Int(0)),
        Assert(Gtxn[3].asset_receiver() == Gtxn[3].sender()), # TODO is this optin check needed - if yes add it to other optins

        # the investor sends 3 txs (app call, pay for shares, shares optin)
        Assert(Gtxn[0].sender() == Gtxn[2].sender()),
        Assert(Gtxn[2].sender() == Gtxn[3].sender()),

        # TODO check that gtxn 1 sender is invest escrow and receiver locking escrow? (add both to state)
        Assert(Gtxn[2].asset_amount() == Mul(Gtxn[1].asset_amount(), tmpl_share_price)), # Paying the correct price for the bought shares

        # save shares on local state
        lock_shares,

        # save the dao id on local state
        save_dao_id,

        Approve()
    )

    program = Cond(
        [Gtxn[0].application_id() == Int(0), handle_create],
        [Global.group_size() == Int(1), handle_optin],
        [Global.group_size() == Int(10), handle_setup_dao],
        [Gtxn[0].application_args[0] == Bytes("unlock"), handle_unlock],
        [Gtxn[0].application_args[0] == Bytes("claim"), handle_claim],
        [Gtxn[0].application_args[0] == Bytes("lock"), handle_lock],
        [Gtxn[0].application_args[0] == Bytes("drain"), handle_drain],
        [Gtxn[0].application_args[0] == Bytes("invest"), handle_invest],
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
