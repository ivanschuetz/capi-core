from pyteal import *

"""Central escrow"""

tmpl_central_app_id = Tmpl.Int("TMPL_CENTRAL_APP_ID")
tmpl_funds_asset_id = Tmpl.Int("TMPL_FUNDS_ASSET_ID")
tmpl_dao_creator = Tmpl.Addr("TMPL_DAO_CREATOR")

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

    is_withdrawal = Global.group_size() == Int(2)
    handle_withdrawal = Seq(
        Assert(Gtxn[0].type_enum() == TxnType.Payment),
        Assert(Gtxn[0].sender() == tmpl_dao_creator),
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].xfer_asset() == tmpl_funds_asset_id),
        Assert(Gtxn[1].asset_receiver() == tmpl_dao_creator),
        Approve()
    )

    is_harvest = And(
        Gtxn[0].type_enum() == TxnType.ApplicationCall,
        Gtxn[0].application_id() == tmpl_central_app_id,
        Gtxn[1].type_enum() == TxnType.AssetTransfer,
    )
    handle_harvest = Seq(
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[1].xfer_asset() == tmpl_funds_asset_id), # the harvested asset is the funds asset 
        Assert(Gtxn[0].sender() == Gtxn[1].asset_receiver()), # app caller is dividend receiver 
        Approve()
    )

################################################
# TODO the branching here is a bit weird - modelled (mostly) after original TEAL
# can this be improved - we use group size and arguments to identify the use cases,
# so we've to branch based on group size / args length
################################################
    is_group_size2 = Global.group_size() == Int(2)
    handle_group_size2 = Cond(
        [is_harvest, handle_harvest],
        [is_withdrawal, handle_withdrawal],
    )
################################################

    program = Cond(
        [is_setup_dao, handle_setup_dao],
        [is_group_size2, handle_group_size2]
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/central_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote central escrow TEAL to: " + path)
