from pyteal import *

"""Customer escrow"""

tmpl_central_app_id = Tmpl.Int("TMPL_CENTRAL_APP_ID")
tmpl_central_escrow_address = Tmpl.Addr("TMPL_CENTRAL_ESCROW_ADDRESS")
tmpl_capi_escrow_address = Tmpl.Addr("TMPL_CAPI_ESCROW_ADDRESS")

GLOBAL_RECEIVED_TOTAL = "ReceivedTotal"
LOCAL_CLAIMED_TOTAL = "ClaimedTotal"
LOCAL_SHARES = "Shares"

def program():
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

        # customer escrow opts-in to funds asset
        Assert(Gtxn[8].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[8].asset_amount() == Int(0)),
        # TODO are these checks neeed for optins? or do we check instead only if sender == receiver? (apply to similar places)
        Assert(Gtxn[8].fee() == Int(0)),
        Assert(Gtxn[8].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[8].rekey_to() == Global.zero_address()),

        Assert(Gtxn[9].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[9].xfer_asset() == Btoi(Gtxn[0].application_args[2])),
        Approve()
    )

    handle_drain = Seq(
        Assert(Global.group_size() == Int(4)),

        # call app to verify amount and update state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].sender() == Gtxn[1].sender()), # same user is calling both apps

        # call capi app to update state
        Assert(Gtxn[1].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[1].on_completion() == OnComplete.NoOp),

        # drain: funds xfer to central escrow
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].asset_amount() > Int(0)),
        Assert(Gtxn[2].asset_receiver() == tmpl_central_escrow_address), # the funds are being drained to the central escrow
        Assert(Gtxn[2].fee() == Int(0)),
        Assert(Gtxn[2].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[2].rekey_to() == Global.zero_address()),

        # pay capi fee: funds xfer to capi escrow
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[3].asset_receiver() == tmpl_capi_escrow_address), # the capi fee is being sent to the capi escrow
        Assert(Gtxn[3].fee() == Int(0)),
        Assert(Gtxn[3].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[3].rekey_to() == Global.zero_address()),

        Approve()
    )

    program = Cond(
        [Global.group_size() == Int(10), handle_setup_dao],
        [Gtxn[0].application_args[0] == Bytes("drain"), handle_drain],
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/customer_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote customer escrow TEAL to: " + path)

