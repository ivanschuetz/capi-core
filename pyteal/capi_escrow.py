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
        Assert(Gtxn[0].type_enum() == TxnType.Payment),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_capi_asset_id),
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].xfer_asset() == tmpl_funds_asset_id),
        Approve()
    )

    is_unlock = And(
        Gtxn[0].type_enum() == TxnType.ApplicationCall,
        Gtxn[0].application_args.length() == Int(1),
        Gtxn[0].application_args[0] == Bytes("unlock"), 
        Gtxn[1].type_enum() == TxnType.AssetTransfer,
    )
    handle_unlock = Seq(
        Assert(Gtxn[0].on_completion() == OnComplete.CloseOut),
        Assert(Gtxn[0].application_id() == tmpl_capi_app_id),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_capi_asset_id),
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is receiving the shares
        Approve()
    )

    is_harvest = Gtxn[0].application_args[0] == Bytes("harvest")
    handle_harvest = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_id() == tmpl_capi_app_id),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_funds_asset_id), # the harvested asset is the funds asset 
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 
        Approve()
    )

################################################
# TODO the branching here is a bit weird - modelled (mostly) after original TEAL
# can this be improved - we use group size and arguments to identify the use cases,
# so we've to branch based on group size / args length
################################################
    is_num_tx0_app_args_1 = Gtxn[0].application_args.length() == Int(1)
    handle_num_tx0_app_args_1 = Cond(
        [is_harvest, handle_harvest],
        [is_unlock, handle_unlock],
    )
    is_group_size2 = Global.group_size() == Int(2)
    handle_group_size2 = Cond(
        [is_num_tx0_app_args_1, handle_num_tx0_app_args_1],
    )
################################################

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
