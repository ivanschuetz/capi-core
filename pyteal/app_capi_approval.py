from pyteal import *

"""App central approval"""

tmpl_share_price = Tmpl.Int("TMPL_SHARE_PRICE")
tmpl_capi_app_id = Tmpl.Int("TMPL_CAPI_APP_ID")
# tmpl_capi_escrow_address = Tmpl.Addr("TMPL_CAPI_ESCROW_ADDRESS")
tmpl_precision = Tmpl.Int("TMPL_PRECISION")
tmpl_capi_share = Tmpl.Int("TMPL_CAPI_SHARE")
tmpl_precision_square = Tmpl.Int("TMPL_PRECISION_SQUARE")
tmpl_capi_share = Tmpl.Int("TMPL_CAPI_SHARE")
tmpl_investors_share = Tmpl.Int("TMPL_INVESTORS_SHARE")
tmpl_share_supply = Tmpl.Int("TMPL_SHARE_SUPPLY")
tmpl_funds_asset_id = Tmpl.Int("TMPL_FUNDS_ASSET_ID")
tmpl_capi_asset_id = Tmpl.Int("TMPL_CAPI_ASSET_ID")

GLOBAL_RECEIVED_TOTAL = "ReceivedTotal"
LOCAL_HARVESTED_TOTAL = "HarvestedTotal"
LOCAL_SHARES = "Shares"

def approval_program():
    is_create = And(
        Gtxn[0].type_enum() == TxnType.ApplicationCall,
        Gtxn[0].application_id() == Int(0),
    )
    handle_create = Approve()

    is_optin = Global.group_size() == Int(1)
    handle_optin = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.OptIn),
        Approve()
    )
    
    total_entitled_harvest_amount = Div(
        Mul(
            Div(
                Mul(App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES)), tmpl_share_supply), 
                tmpl_share_supply
            ), 
            App.globalGet(Bytes(GLOBAL_RECEIVED_TOTAL))
        ), 
        tmpl_precision
    )

    # Calculates entitled harvest based on LOCAL_SHARES and LOCAL_HARVESTED_TOTAL.
    # Expects harvester to be the gtxn 0 sender. 
    entitled_harvest_amount = Minus(total_entitled_harvest_amount, App.localGet(Gtxn[0].sender(), Bytes(LOCAL_HARVESTED_TOTAL)))
    wants_to_harvest_less_or_eq_to_entitled_amount = Ge(entitled_harvest_amount, Gtxn[1].asset_amount())

    is_harvest = Gtxn[0].application_args[0] == Bytes("harvest")
    handle_harvest = Seq(
        # app call to verify and set dividend
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 

        # xfer to transfer dividend to investor
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_funds_asset_id), # the harvested asset is the funds asset 

        # verify dividend amount is correct
        Assert(wants_to_harvest_less_or_eq_to_entitled_amount),

        # update local state with retrieved dividend
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

    is_unlock = And(
        Gtxn[0].type_enum() == TxnType.ApplicationCall,
        Gtxn[0].application_args.length() == Int(1),
        Gtxn[0].application_args[0] == Bytes("unlock"), 

        Gtxn[1].type_enum() == TxnType.AssetTransfer,
    )
    handle_unlock = Seq(
        # app call to opt-out
        Assert(Gtxn[0].on_completion() == OnComplete.CloseOut),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is receiving the shares

        # xfer to get the capi assets
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_capi_asset_id),
        Assert(Gtxn[1].asset_amount() == App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES))), # unlocked amount == owned shares
        Approve()
    )
    
    is_lock = Global.group_size() == Int(2)
   
    handle_lock = Seq(
        # app call to update state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].sender() == Gtxn[1].sender()),

        # send capi assets to capi escrow
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_capi_asset_id),
        Assert(Gtxn[1].asset_amount() > Int(0)),

        # set / increment share count local state
        App.localPut( 
            Gtxn[0].sender(), 
            Bytes(LOCAL_SHARES), 
            Add(
                App.localGet(Gtxn[0].sender(), Bytes(LOCAL_SHARES)),
                Gtxn[1].asset_amount()
            )
        ),

        # set already harvested local state
        App.localPut( 
            Gtxn[0].sender(), 
            Bytes(LOCAL_HARVESTED_TOTAL), 
            # NOTE that this sets HarvestedTotal to the entitled amount each time that the investor buys/locks shares
            # meaning that investors may lose pending dividend by buying or locking new shares
            # TODO improve? - a non TEAL way could be to just automatically retrieve pending dividend in the same group 
            # see more notes in old repo
            entitled_harvest_amount
            # Gtxn[1].asset_amount()
        ),

        Approve()
    )

    is_num_tx0_app_args_1 = Gtxn[0].application_args.length() == Int(1)
    handle_num_tx0_app_args_1 = Cond(
        [is_harvest, handle_harvest],
        [is_unlock, handle_unlock],
    )
    is_group_size2 = Global.group_size() == Int(2)
    handle_group_size2 = Cond(
        [is_num_tx0_app_args_1, handle_num_tx0_app_args_1],
        [is_lock, handle_lock],
    )

    is_drain = Global.group_size() == Int(4)
    
    handle_drain = Seq(
        # call app to verify amount and update state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # same user is calling both apps

        # call capi app to update state
        Assert(Gtxn[1].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[1].on_completion() == OnComplete.NoOp),

        # drain: funds xfer to central escrow
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].xfer_asset() == tmpl_funds_asset_id),

        # pay capi fee: funds xfer to capi escrow
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[3].xfer_asset() == tmpl_funds_asset_id),
        # Assert(Gtxn[3].asset_receiver() == tmpl_capi_escrow_address),

        # update total capi fee received
        App.globalPut(
            Bytes(GLOBAL_RECEIVED_TOTAL), 
            Add(App.globalGet(Bytes(GLOBAL_RECEIVED_TOTAL)), Gtxn[3].asset_amount())
        ),

        Approve()
    )
    
    program = Cond(
        [is_create, handle_create],
        [is_optin, handle_optin],
        [is_group_size2, handle_group_size2],
        [is_drain, handle_drain],
    )

    return compileTeal(program, Mode.Application, version=5)

def clear_program():
    return compileTeal(Int(1), Mode.Application, version=5)
 
def export(path, output):
   with open(path, "w") as f:
    # print(output)
    f.write(output)
    print("Wrote TEAL to: " + path)

export("teal_template/app_capi_approval.teal", approval_program())
export("teal/app_capi_clear.teal", clear_program())

print("Done! Wrote capi approval and clear TEAL")
