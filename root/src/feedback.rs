use educe::Educe;
use thiserror::Error;
use crate::framework::RoutingSystem;

/// These are critical errors that may affect the stability or security of the application
#[derive(Error)]
#[derive(Educe)]
#[educe(Debug)]
pub enum RoutingError<T: RoutingSystem + ?Sized> {
    /// This can either be due to packet forgery, MITM, or system misconfiguration
    #[error("Rejected packet over link, MAC Validation Failed.")]
    MACValidationFail{
        link: T::Link
    }
}

/// Although this is an error enum, these should be treated as warnings.
#[derive(Error)]
#[derive(Educe)]
#[educe(Debug)]
pub enum RoutingWarning<T: RoutingSystem + ?Sized>{
    /// The metric over a link should never be zero (this may result in routing loops!)
    /// If this warning is triggered, root will automatically set the metric to 1.
    MetricIsZero{
        link: T::Link
    },
    /// This warning is triggered when a neighbour requests a seqno update (that has passed MAC validation), where the requested seqno > cur_seqno + 1.
    /// Depending on TRUST_RESYNC_SEQNO, root will automatically trust this seqno request and synchronize the seqno with the requested seqno.
    /// NOTE: This might be an indication of node data loss!
    DesynchronizedSeqno{
        old_seqno: u16,
        new_seqno: u16
    }
}