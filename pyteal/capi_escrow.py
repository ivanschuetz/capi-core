from pyteal import *

"""Capi escrow"""

tmpl_capi_app_id = Tmpl.Int("TMPL_CAPI_APP_ID")
tmpl_funds_asset_id = Tmpl.Int("TMPL_FUNDS_ASSET_ID")
tmpl_capi_asset_id = Tmpl.Int("TMPL_CAPI_ASSET_ID")

GLOBAL_RECEIVED_TOTAL = "ReceivedTotal"
LOCAL_CLAIMED_TOTAL = "ClaimedTotal"
LOCAL_SHARES = "Shares"

def program():
    handle_setup = Seq(
        # capi creator funds escrow with min balance
        Assert(Gtxn[0].type_enum() == TxnType.Payment),
        Assert(Gtxn[0].close_remainder_to() == Global.zero_address()),
        Assert(Gtxn[0].rekey_to() == Global.zero_address()),

        # escrow opt-ins to capi asset
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_capi_asset_id),
        Assert(Gtxn[1].fee() == Int(0)),
        Assert(Gtxn[1].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[1].rekey_to() == Global.zero_address()),

        # escrow opt-ins to funds asset
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].xfer_asset() == tmpl_funds_asset_id),
        Assert(Gtxn[2].fee() == Int(0)),
        Assert(Gtxn[2].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[2].rekey_to() == Global.zero_address()),

        Approve()
    )

    handle_unlock = Seq(
        Assert(Global.group_size() == Int(2)),

        # app call to opt out
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].application_args.length() == Int(1)),
        Assert(Gtxn[0].on_completion() == OnComplete.CloseOut),
        Assert(Gtxn[0].application_id() == tmpl_capi_app_id),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is receiving the shares

        # xfer to get the shares
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == tmpl_capi_asset_id),
        Assert(Gtxn[1].fee() == Int(0)),
        Assert(Gtxn[1].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[1].rekey_to() == Global.zero_address()),

        Approve()
    )

    handle_claim = Seq(
        Assert(Global.group_size() == Int(2)),

        # app call to calculate and set dividend
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_id() == tmpl_capi_app_id),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 

        # xfer with dividend
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == tmpl_funds_asset_id), # the claimed asset is the funds asset 
        Assert(Gtxn[1].fee() == Int(0)),
        Assert(Gtxn[1].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[1].rekey_to() == Global.zero_address()),

        Approve()
    )

    program = Cond(
        [Global.group_size() == Int(10), handle_setup],
        [Global.group_size() == Int(3), handle_setup],
        [Gtxn[0].application_args[0] == Bytes("unlock"), handle_unlock],
        [Gtxn[0].application_args[0] == Bytes("claim"), handle_claim],
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/capi_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote capi escrow TEAL to: " + path)
