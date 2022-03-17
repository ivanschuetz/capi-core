from pyteal import *

"""Investing escrow"""

tmpl_share_price = Tmpl.Int("TMPL_SHARE_PRICE")
tmpl_central_app_id = Tmpl.Int("TMPL_CENTRAL_APP_ID")
tmpl_funds_asset_id = Tmpl.Int("TMPL_FUNDS_ASSET_ID")
tmpl_dao_creator = Tmpl.Addr("TMPL_DAO_CREATOR")
tmpl_shares_asset_id = Tmpl.Int("TMPL_SHARES_ASSET_ID")
tmpl_locking_escrow_address = Tmpl.Addr("TMPL_LOCKING_ESCROW_ADDRESS")
tmpl_central_escrow_address = Tmpl.Addr("TMPL_CENTRAL_ESCROW_ADDRESS")

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

        # investing escrow opts-in to shares
        Assert(Gtxn[6].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[6].asset_amount() == Int(0)),
        Assert(Gtxn[6].fee() == Int(0)),
        Assert(Gtxn[6].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[6].rekey_to() == Global.zero_address()),

        Assert(Gtxn[7].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[7].asset_amount() == Int(0)),

        Assert(Gtxn[8].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[8].asset_amount() == Int(0)),

        Assert(Gtxn[9].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[9].xfer_asset() == Btoi(Gtxn[0].application_args[2])),

        Approve()
    )

    handle_invest = Seq(
        Assert(Global.group_size() == Int(4)),

        # app call to initialize shares state
        Assert(Gtxn[0].type_enum() == TxnType.ApplicationCall),
        Assert(Gtxn[0].on_completion() == OnComplete.NoOp),
        Assert(Gtxn[0].application_id() == tmpl_central_app_id),
        Assert(Gtxn[0].application_args.length() == Int(2)),

        # shares xfer to investor
        Assert(Gtxn[1].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[1].asset_amount() > Int(0)),
        Assert(Gtxn[1].xfer_asset() == tmpl_shares_asset_id),
        Assert(Gtxn[1].asset_receiver() == tmpl_locking_escrow_address), 
        Assert(Gtxn[1].fee() == Int(0)),
        Assert(Gtxn[1].asset_close_to() == Global.zero_address()),
        Assert(Gtxn[1].rekey_to() == Global.zero_address()),

        # investor pays for shares: funds xfer to central escrow
        Assert(Gtxn[2].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[2].asset_amount() > Int(0)),
        Assert(Gtxn[2].xfer_asset() == tmpl_funds_asset_id), 
        Assert(Gtxn[2].asset_receiver() == tmpl_central_escrow_address),

        # investor opts-in to shares 
        Assert(Gtxn[3].type_enum() == TxnType.AssetTransfer),
        Assert(Gtxn[3].xfer_asset() == tmpl_shares_asset_id),
        Assert(Gtxn[3].asset_amount() == Int(0)),
        Assert(Gtxn[3].asset_receiver() == Gtxn[3].sender()), # TODO is this optin check needed - if yes add it to other optins

        # the investor sends 3 txs (app call, pay for shares, shares optin)
        Assert(Gtxn[0].sender() == Gtxn[2].sender()),
        Assert(Gtxn[2].sender() == Gtxn[3].sender()),
        # TODO check that gtxn 1 sender is invest escrow and receiver locking escrow? (add both to state)
        Assert(Gtxn[2].asset_amount() == Mul(Gtxn[1].asset_amount(), tmpl_share_price)), # Paying the correct price for the bought shares

        Approve()
    )

    program = Cond(
        [Global.group_size() == Int(10), handle_setup_dao],
        [Gtxn[0].application_args[0] == Bytes("invest"), handle_invest],
    )

    return compileTeal(program, Mode.Signature, version=5)

path = 'teal_template/investing_escrow.teal'
with open(path, 'w') as f:
    output = program()
    # print(output)
    f.write(output)
    print("Done! Wrote investing escrow TEAL to: " + path)