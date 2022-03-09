from pyteal import *

"""Customer escrow"""

tmpl_central_app_id = Tmpl.Int("TMPL_CENTRAL_APP_ID")
tmpl_central_escrow_address = Tmpl.Addr("TMPL_CENTRAL_ESCROW_ADDRESS")
tmpl_capi_escrow_address = Tmpl.Addr("TMPL_CAPI_ESCROW_ADDRESS")

GLOBAL_RECEIVED_TOTAL = "ReceivedTotal"
LOCAL_HARVESTED_TOTAL = "HarvestedTotal"
LOCAL_SHARES = "Shares"

def program():
    is_setup_dao = Global.group_size() == Int(10)
    handle_setup_dao = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_id() == tmpl_central_app_id),
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
        Approve()
    )

    is_drain = And(Global.group_size() == Int(4))
    handle_drain = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall), # dao app call
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[1].type_enum() == TxnType.ApplicationCall), # capi app call
        Assert(Gtxn[1].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer), # drain
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer), # capi share
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # same user is calling both apps
        Assert(Gtxn[2].asset_receiver() == tmpl_central_escrow_address), # the funds are being drained to the central escrow
        Assert(Gtxn[3].asset_receiver() == tmpl_capi_escrow_address), # the capi fee is being sent to the capi escrow
        Approve()
    )

################################################
# TODO the branching here is a bit weird - modelled (mostly) after original TEAL
# can this be improved - we use group size and arguments to identify the use cases,
# so we've to branch based on group size / args length
################################################
    is_group_size4 = Global.group_size() == Int(4)
    handle_group_size4 = Cond(
        [is_drain, handle_drain], 
    )
################################################

    program = Cond(
        [is_setup_dao, handle_setup_dao],
        [is_group_size4, handle_group_size4]
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/customer_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! output: " + path)

