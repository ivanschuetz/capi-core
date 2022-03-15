from pyteal import *

"""Capi escrow"""

tmpl_capi_app_id = Tmpl.Int("TMPL_CAPI_APP_ID")
tmpl_funds_asset_id = Tmpl.Int("TMPL_FUNDS_ASSET_ID")
tmpl_capi_asset_id = Tmpl.Int("TMPL_CAPI_ASSET_ID")

GLOBAL_RECEIVED_TOTAL = "ReceivedTotal"
LOCAL_HARVESTED_TOTAL = "HarvestedTotal"
LOCAL_SHARES = "Shares"

def program():
    is_setup = And(
        Global.group_size() == Int(3),

    )
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

    is_unlock = And(
        Gtxn[0].type_enum() == TxnType.ApplicationCall,
        Gtxn[0].application_args.length() == Int(1),
        Gtxn[0].application_args[0] == Bytes("unlock"), 

        Gtxn[1].type_enum() == TxnType.AssetTransfer,
    )
    handle_unlock = Seq(
        # app call to opt out
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

    is_harvest = Gtxn[0].application_args[0] == Bytes("harvest")
    handle_harvest = Seq(
        # app call to calculate and set dividend
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_id() == tmpl_capi_app_id),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 

        # xfer with dividend
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == tmpl_funds_asset_id), # the harvested asset is the funds asset 
        Assert(Gtxn[1].fee() == Int(0)),
        Assert(Gtxn[1].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[1].rekey_to() == Global.zero_address()),

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
    )

    program = Cond(
        [is_setup, handle_setup],
        [is_group_size2, handle_group_size2],
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/capi_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote capi escrow TEAL to: " + path)
