use algonaut::{
    algod::v2::Algod,
    core::ToMsgPack,
    error::ServiceError,
    model::algod::v2::DryrunResponse,
    transaction::SignedTransaction,
    util::dryrun_printer::{
        app_trace_with_config, create_dryrun, lsig_trace_with_config, BytesFormat, MaxColumnWidths,
        StackPrinterConfig,
    },
};

use crate::debug_msg_pack_submit_par::write_bytes_to_tmp_file;

fn print_config() -> StackPrinterConfig {
    StackPrinterConfig {
        max_column_widths: MaxColumnWidths {
            stack: 1000,
            ..MaxColumnWidths::default()
        },
        top_of_stack_first: false,
        bytes_format: BytesFormat::AddressOrHex,
    }
}

#[allow(dead_code)]
pub async fn dryrun_all(
    algod: &Algod,
    signed_txs: &[SignedTransaction],
) -> Result<(), ServiceError> {
    let res = dryrun_req(algod, signed_txs).await?;

    print!("???? res: {:?}", res);

    if let Some(error) = &res.error {
        return Err(ServiceError::Msg(format!(
            "Dryrun error: {error}. Complete response: {:?}",
            res
        )));
    }
    trace_all(&res).await?;
    Ok(())
}

async fn trace_all(res: &DryrunResponse) -> Result<(), ServiceError> {
    let config = print_config();
    trace_app(&res, &config).await?;
    trace_lsig(&res, &config).await?;
    Ok(())
}

async fn dryrun_req(
    algod: &Algod,
    signed_txs: &[SignedTransaction],
) -> Result<DryrunResponse, ServiceError> {
    let req = create_dryrun(algod, &signed_txs).await?;

    let msg_pack = req.to_msg_pack()?;
    write_bytes_to_tmp_file(&msg_pack);

    algod.dryrun_teal(&req).await
}

async fn trace_app(res: &DryrunResponse, config: &StackPrinterConfig) -> Result<(), ServiceError> {
    for tx in &res.txns {
        println!("{}", app_trace_with_config(&tx, config)?);
    }
    Ok(())
}

async fn trace_lsig(res: &DryrunResponse, config: &StackPrinterConfig) -> Result<(), ServiceError> {
    for tx in &res.txns {
        println!("{}", lsig_trace_with_config(&tx, config)?);
    }
    Ok(())
}
