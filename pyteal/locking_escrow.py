from pyteal import *

"""Locking escrow"""

tmpl_central_app_id = Tmpl.Int("TMPL_CENTRAL_APP_ID")
tmpl_shares_asset_id = Tmpl.Int("TMPL_SHARES_ASSET_ID")

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

        # locking escrow opt-in to shares
        Assert(Gtxn[5].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[5].asset_amount() == Int(0)),
        Assert(Gtxn[5].fee() == Int(0)),
        Assert(Gtxn[5].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[5].rekey_to() == Global.zero_address()),

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

    handle_unlock = Seq(
        Assert(Global.group_size() == Int(2)),

        # app call to opt-out
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_id() == tmpl_central_app_id),
        Assert(Gtxn[0].on_completion() == OnComplete.CloseOut),

        # shares xfer to investor
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_shares_asset_id),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].asset_receiver() == Gtxn[0].sender()),
        Assert(Gtxn[1].fee() == Int(0)),
        Assert(Gtxn[1].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[1].rekey_to() == Global.zero_address()),

        Approve()
    )
 
    program = Cond(
        [Global.group_size() == Int(10), handle_setup_dao],
        [Gtxn[0].application_args[0] == Bytes("unlock"), handle_unlock],
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/locking_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote locking escrow TEAL to: " + path)
